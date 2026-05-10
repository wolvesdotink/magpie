mod accessibility;
mod audio;
mod commands;
mod constants;
mod correction;
mod correction_detector;
mod events;
mod hotkey;
mod models;
mod output;
mod permissions;
mod settings;
mod state;
mod transcription;
mod tray;
mod overlay;
mod recording;
mod vocabulary;

use std::sync::Arc;

#[allow(unused_imports)]
use tauri::Manager;
use tokio::sync::mpsc;

use crate::recording::RecordingCommand;
use crate::state::AppState;

pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init());

    // Updater + process are desktop-only. The frontend invokes these through
    // @tauri-apps/plugin-updater and @tauri-apps/plugin-process; we only need
    // to register them on the Rust side here. Updates are signed/verified
    // against the public key in tauri.conf.json (plugins.updater.pubkey) and
    // fetched from the configured endpoints (latest.json on GitHub releases).
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }

    builder
        .manage(Arc::new(AppState::new()))
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::toggle_recording,
            commands::get_app_state,
            commands::get_available_models,
            commands::get_downloaded_models,
            commands::download_model,
            commands::select_model,
            commands::delete_model_file,
            commands::check_permissions,
            commands::request_microphone_permission,
            commands::open_microphone_settings,
            commands::open_accessibility_settings,
            commands::request_input_monitoring_permission,
            commands::open_input_monitoring_settings,
            commands::get_settings,
            commands::update_settings,
            commands::get_available_correction_models,
            commands::get_downloaded_correction_models,
            commands::download_correction_model,
            commands::select_correction_model,
            commands::delete_correction_model_file,
            commands::restart_fn_key_monitor,
            commands::get_fn_key_monitor_status,
            commands::restart_app,
            commands::get_vocabulary,
            commands::add_vocabulary_entry,
            commands::remove_vocabulary_entry,
            commands::clear_vocabulary,
        ])
        .on_window_event(|window, event| {
            match window.label() {
                "main" => match event {
                    tauri::WindowEvent::Focused(false) => {
                        use std::sync::atomic::Ordering;
                        let state = window.app_handle().state::<Arc<AppState>>();

                        // During setup the window shouldn't auto-hide on blur
                        let in_setup = state
                            .settings
                            .lock()
                            .map(|s| !s.setup_complete)
                            .unwrap_or(false);

                        // One-shot suppression (e.g. opening System Preferences)
                        let suppressed =
                            state.suppress_hide.swap(false, Ordering::SeqCst);

                        if !in_setup && !suppressed {
                            let _ = window.hide();
                        }
                    }
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    _ => {}
                },
                "settings" => {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
                _ => {}
            }
        })
        .setup(|app| {
            // Tauri's setup closure runs inside tao's `did_finish_launching`,
            // which is an `extern "C"` Objective-C callback. Any panic here
            // would unwind across the C ABI boundary and abort the process
            // with "panic in a function that cannot unwind". We wrap the
            // entire setup body in catch_unwind to convert panics into errors.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                setup_app(app)
            }));
            match result {
                Ok(inner) => inner,
                Err(panic_payload) => {
                    let msg = panic_payload
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| panic_payload.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "unknown panic".to_string());
                    let err_msg = format!("Panic during app setup: {}", msg);
                    eprintln!("{}", err_msg);
                    Err(err_msg.into())
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Error while running Magpie");
}

/// The actual setup logic, extracted so it can be wrapped in catch_unwind.
fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Hide dock icon - make this a menu bar only app
    #[cfg(target_os = "macos")]
    #[allow(deprecated)]
    {
        use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicy};
        unsafe {
            let ns_app = NSApp();
            ns_app.setActivationPolicy_(
                NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory,
            );
        }
    }

    // Set up system tray
    tray::setup_tray(app)?;

    // Configure overlay window (click-through, centering)
    overlay::setup_overlay(&app.handle());

    // Force settings window to be fully transparent so the
    // CSS outer padding shows through as empty space.
    if let Some(settings_win) = app.get_webview_window("settings") {
        // Tauri API — sets both window and webview background
        let _ = settings_win.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));

    } else {
        log::warn!("Settings window not found during setup");
    }

    // Clean up stale temp files from interrupted downloads
    cleanup_stale_downloads();

    // Try to load previously selected model
    let state = app.state::<Arc<AppState>>();
    try_load_last_model(&state);

    // Try to load previously selected correction model
    try_load_last_correction_model(&state);

    // Show the main window on startup if anything blocks normal operation:
    // no model loaded, or any of the three required permissions revoked.
    // App.vue routes to the permissions guide when any perm is missing, so
    // surfacing the window is enough — without this, a returning user who
    // revoked a permission would press Fn and get no signal.
    {
        let has_model = state
            .whisper_context
            .lock()
            .map(|ctx| ctx.is_some())
            .unwrap_or(false);
        let perms = commands::check_permissions();
        let missing_perms =
            !perms.microphone || !perms.accessibility || !perms.input_monitoring;
        if !has_model || missing_perms {
            if !has_model {
                log::info!("No model loaded — showing setup window");
            } else {
                log::info!(
                    "Missing permissions on startup (mic={}, accessibility={}, input_monitoring={}) — showing window",
                    perms.microphone,
                    perms.accessibility,
                    perms.input_monitoring,
                );
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }

    // Create a channel to serialize recording commands.
    // This guarantees stop always waits for start to complete.
    let (tx, mut rx) = mpsc::unbounded_channel::<RecordingCommand>();

    // Store the sender so restart_fn_key_monitor can clone it later
    if let Ok(mut guard) = state.recording_tx.lock() {
        *guard = Some(tx.clone());
    }

    // Proactively request Input Monitoring permission at startup. On first
    // launch this triggers the macOS TCC prompt AND registers the app in
    // System Settings → Privacy & Security → Input Monitoring so the toggle
    // becomes available. On subsequent launches it's a cheap no-op that just
    // returns the current state. Doing this BEFORE starting the Fn key
    // monitor means the monitor's preflight check sees the correct state on
    // first run. Fn events go through CGEventTap which is gated by Input
    // Monitoring on macOS 10.15+, NOT Accessibility.
    let input_monitoring_granted = permissions::request_input_monitoring_access();
    log::info!(
        "Input Monitoring permission: {}",
        if input_monitoring_granted { "granted" } else { "NOT granted — user must enable in System Settings" }
    );

    // Start Fn key monitor — pass channel sender directly (no Tauri events)
    let app_handle = app.handle().clone();
    let activation_mode = state
        .settings
        .lock()
        .map(|s| s.activation_mode.clone())
        .unwrap_or_default();
    let hotkey_tx = tx.clone();
    let (monitor_handle, monitor_ok) =
        hotkey::start_fn_key_monitor(app_handle.clone(), hotkey_tx, activation_mode);
    if !monitor_ok {
        log::warn!(
            "Fn key monitor failed to start — Input Monitoring permission likely missing. \
             Banner will direct the user to System Settings."
        );
    }
    if let Ok(mut guard) = state.fn_key_monitor.lock() {
        *guard = Some(monitor_handle);
    }

    // Fn monitor watchdog. The tap callback tries to re-enable itself when
    // macOS disables the tap (see hotkey.rs). If that fast-path fails (e.g.
    // the tap was disabled before the mach port was stored, or re-enable
    // didn't take), the callback sets a `needs_restart` flag. This watchdog
    // polls the flag every 10 s and tears down + restarts the monitor when
    // set. It also gives us a recovery path for zombie taps that stop
    // delivering events without triggering a disable event.
    let watchdog_app = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        use std::time::Duration;
        use tokio::time::sleep;

        // Initial grace period so the monitor can finish spinning up before
        // the first health check.
        sleep(Duration::from_secs(10)).await;

        loop {
            sleep(Duration::from_secs(10)).await;

            let state = watchdog_app.state::<Arc<AppState>>();
            let needs_restart = state
                .fn_key_monitor
                .lock()
                .ok()
                .and_then(|guard| guard.as_ref().map(|h| h.needs_restart()))
                .unwrap_or(false);

            if needs_restart {
                log::info!(
                    "Fn key monitor watchdog: needs_restart flag set — restarting"
                );
                match commands::restart_fn_key_monitor_inner(
                    &watchdog_app,
                    state.inner(),
                ) {
                    Ok(true) => {
                        log::info!("Fn key monitor watchdog: restart succeeded");
                    }
                    Ok(false) => {
                        log::warn!(
                            "Fn key monitor watchdog: restart returned false \
                             (tap creation failed — likely missing accessibility)"
                        );
                    }
                    Err(e) => {
                        log::error!("Fn key monitor watchdog: restart error: {}", e);
                    }
                }
            }
        }
    });

    // Spawn a single consumer task that processes commands sequentially
    let consumer_app = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            let state = consumer_app.state::<Arc<AppState>>();
            match cmd {
                RecordingCommand::Start => {
                    if !state.is_recording() && !state.is_processing() {
                        if let Err(e) = commands::start_recording(
                            consumer_app.clone(),
                            state.clone(),
                        )
                        .await
                        {
                            log::error!("Failed to start recording: {}", e);
                        }
                    } else {
                        log::debug!(
                            "Ignoring start command: recording={}, processing={}",
                            state.is_recording(),
                            state.is_processing()
                        );
                    }
                }
                RecordingCommand::Stop => {
                    if state.is_recording() {
                        if let Err(e) = commands::stop_recording(
                            consumer_app.clone(),
                            state.clone(),
                        )
                        .await
                        {
                            log::error!("Failed to stop recording: {}", e);
                        }
                    } else {
                        log::debug!("Ignoring stop command: not recording");
                    }
                }
                RecordingCommand::Toggle => {
                    if state.is_recording() {
                        if let Err(e) = commands::stop_recording(
                            consumer_app.clone(),
                            state.clone(),
                        )
                        .await
                        {
                            log::error!("Failed to stop recording: {}", e);
                        }
                    } else if !state.is_processing() {
                        if let Err(e) = commands::start_recording(
                            consumer_app.clone(),
                            state.clone(),
                        )
                        .await
                        {
                            log::error!("Failed to start recording: {}", e);
                        }
                    }
                }
            }
        }
    });

    // Register fallback global shortcut (Cmd+Shift+Space)
    let shortcut_tx = tx.clone();
    register_global_shortcut(app, shortcut_tx)?;

    Ok(())
}

/// Try to load the last used model on startup.
///
/// The actual whisper-rs FFI work runs in a dedicated thread so that a C++
/// exception escaping the FFI boundary (which `catch_unwind` cannot catch)
/// aborts only that thread instead of the entire application.
fn try_load_last_model(state: &Arc<AppState>) {
    let selected = match state.settings.lock() {
        Ok(settings) => settings.selected_model.clone(),
        Err(e) => {
            log::error!("Settings mutex poisoned in try_load_last_model: {}", e);
            return;
        }
    };

    let (model_id, path) = match selected {
        Some(id) => {
            let info = match models::registry::find_model(&id) {
                Some(i) => i,
                None => return,
            };
            let p = match models::storage::model_path(&info.filename) {
                Ok(p) if p.exists() => p,
                _ => return,
            };
            (id, p)
        }
        None => return,
    };

    // Run the FFI-heavy work on a separate thread.  If whisper.cpp throws a
    // C++ exception that escapes through the extern-"C" boundary, it will
    // abort *this* thread rather than the main thread, keeping the app alive.
    let path_clone = path.clone();
    let load_handle = std::thread::Builder::new()
        .name("whisper-load".into())
        .spawn(move || -> Result<whisper_rs::WhisperContext, String> {
            transcription::engine::load_model(&path_clone)
                .map_err(|e| format!("{}", e))
        });

    let handle = match load_handle {
        Ok(h) => h,
        Err(e) => {
            log::error!("Failed to spawn whisper-load thread: {}", e);
            return;
        }
    };

    match handle.join() {
        Ok(Ok(ctx)) => {
            if let Ok(mut whisper_ctx) = state.whisper_context.lock() {
                *whisper_ctx = Some(ctx);
            }
            if let Ok(mut model_path) = state.current_model_path.lock() {
                *model_path = Some(path);
            }
            log::info!("Auto-loaded model: {}", model_id);
        }
        Ok(Err(e)) => {
            log::error!("Failed to auto-load model {}: {}", model_id, e);
        }
        Err(_) => {
            // The thread panicked or was aborted (e.g. foreign C++ exception).
            // Clear the selection so we don't crash-loop, but keep the file.
            log::error!(
                "whisper-load thread crashed while loading model {}. \
                 Clearing selection to prevent crash loop.",
                model_id
            );
            if let Ok(mut settings) = state.settings.lock() {
                settings.selected_model = None;
                let _ = settings.save();
            }
        }
    }
}

/// Remove leftover `.downloading` temp files from interrupted downloads
fn cleanup_stale_downloads() {
    let dir = match models::storage::models_dir() {
        Ok(d) => d,
        Err(e) => {
            log::warn!("Could not access models directory for cleanup: {}", e);
            return;
        }
    };
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".downloading") {
                log::info!("Removing stale temp file: {}", name);
                if let Err(e) = std::fs::remove_file(entry.path()) {
                    log::warn!("Failed to remove stale temp file {}: {}", name, e);
                }
            }
        }
    }
}

/// Try to load the last used correction model on startup.
///
/// The actual llama.cpp FFI work runs in a dedicated thread so that a C++
/// exception escaping the FFI boundary (which `catch_unwind` cannot catch)
/// aborts only that thread instead of the entire application.
fn try_load_last_correction_model(state: &Arc<AppState>) {
    let selected = match state.settings.lock() {
        Ok(settings) => settings.selected_correction_model.clone(),
        Err(e) => {
            log::error!("Settings mutex poisoned in try_load_last_correction_model: {}", e);
            return;
        }
    };

    let (model_id, path) = match selected {
        Some(id) => {
            let info = match correction::registry::find_correction_model(&id) {
                Some(i) => i,
                None => return,
            };
            let p = match models::storage::model_path(&info.filename) {
                Ok(p) if p.exists() => p,
                _ => return,
            };
            (id, p)
        }
        None => return,
    };

    // Run the FFI-heavy work on a separate thread.  If llama.cpp throws a C++
    // exception that escapes through the extern-"C" boundary, it will abort
    // *this* thread rather than the main thread, keeping the app alive.
    let path_clone = path.clone();
    let load_handle = std::thread::Builder::new()
        .name("llm-load".into())
        .spawn(move || -> Result<(llama_cpp_2::llama_backend::LlamaBackend, llama_cpp_2::model::LlamaModel), String> {
            let backend = llama_cpp_2::llama_backend::LlamaBackend::init()
                .map_err(|e| format!("Failed to init llama backend: {:?}", e))?;
            let model = correction::engine::load_correction_model(&backend, &path_clone)
                .map_err(|e| format!("Failed to load correction model: {}", e))?;
            Ok((backend, model))
        });

    let handle = match load_handle {
        Ok(h) => h,
        Err(e) => {
            log::error!("Failed to spawn llm-load thread: {}", e);
            return;
        }
    };

    // Wait for the loader thread.  A panic or C++ abort in the thread
    // surfaces as an Err from join() – we handle it gracefully.
    match handle.join() {
        Ok(Ok((backend, model))) => {
            // Store the loaded backend + model into shared state.
            if let Ok(mut bg) = state.llama_backend.lock() {
                *bg = Some(backend);
            }
            if let Ok(mut cm) = state.correction_model.lock() {
                *cm = Some(model);
            }
            if let Ok(mut cp) = state.current_correction_model_path.lock() {
                *cp = Some(path);
            }
            log::info!("Auto-loaded correction model: {}", model_id);
        }
        Ok(Err(e)) => {
            // Clean Rust error – model file is likely valid, don't delete it.
            log::error!("Failed to auto-load correction model {}: {}", model_id, e);
            if let Ok(mut settings) = state.settings.lock() {
                settings.selected_correction_model = None;
                let _ = settings.save();
            }
        }
        Err(_) => {
            // The thread panicked or was aborted (e.g. foreign C++ exception).
            // Clear the selection so we don't crash-loop, but keep the file.
            log::error!(
                "llm-load thread crashed while loading correction model {}. \
                 Clearing selection to prevent crash loop.",
                model_id
            );
            if let Ok(mut settings) = state.settings.lock() {
                settings.selected_correction_model = None;
                let _ = settings.save();
            }
        }
    }
}

/// Register fallback keyboard shortcut, routing through the serialized command channel
fn register_global_shortcut(
    app: &tauri::App,
    tx: mpsc::UnboundedSender<RecordingCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let shortcut: Shortcut = "CmdOrCtrl+Shift+Space".parse()?;

    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(e) = tx.send(RecordingCommand::Toggle) {
                    log::error!("Failed to send toggle command: {}", e);
                }
            }
        })?;

    Ok(())
}
