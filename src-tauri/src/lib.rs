// Phase 1 gate: production code may not call `.unwrap()`. Test code is
// exempt via `allow-unwrap-in-tests = true` in src-tauri/clippy.toml.
// To trip this lint deliberately, use `expect("invariant: …")` instead.
#![deny(clippy::unwrap_used)]

mod accessibility;
mod audio;
mod command_error;
mod commands;
mod constants;
mod correction;
mod correction_detector;
mod events;
mod frontmost_app;
mod history;
mod hotkey;
#[cfg(target_os = "macos")]
mod launch_at_login;
mod model_loading;
mod models;
mod output;
mod overlay;
mod permissions;
mod profiles;
mod recording;
mod resolver;
mod running_apps;
mod settings;
mod setup;
mod state;
mod styles;
mod transcription;
mod tray;
mod vocabulary;

use std::sync::Arc;

use crate::state::AppState;

// Re-export the public load helpers used by `commands::*` via `crate::*` paths,
// so the call sites stay short.
pub use model_loading::{flush_pending_reload, load_with_self_test};

/// Default global shortcut used when the user has not configured a custom one.
pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+Space";

/// Append a startup-error breadcrumb to `~/Library/Logs/com.magpie.app/setup-panic.log`.
/// Best-effort: any I/O failure is swallowed because the alternative is
/// losing the message entirely. Used for failures so early that
/// `tauri-plugin-log` hasn't initialized its file writer yet — without
/// this, a Finder-launched app that panics in setup leaves no on-disk trace
/// because stderr is `/dev/null` for GUI launches.
#[cfg(target_os = "macos")]
fn write_startup_breadcrumb(msg: &str) {
    use std::io::Write;

    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let dir = std::path::PathBuf::from(home).join("Library/Logs/com.magpie.app");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("setup-panic.log");

    let timestamp = chrono::Local::now().to_rfc3339();
    let entry = format!("[{timestamp}] {msg}\n");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(entry.as_bytes());
    }
}

#[cfg(not(target_os = "macos"))]
fn write_startup_breadcrumb(_msg: &str) {}

/// Install a `tracing` subscriber for new structured-logging code. Filter
/// is taken from `MAGPIE_LOG` if set, else `RUST_LOG`, else `info` for the
/// app crate and `warn` for everything else.
///
/// Notes on coexistence with `log`:
///   - Existing `log::info!`/`debug!` etc. calls continue to flow through
///     `tauri-plugin-log` (file + console). They do NOT show up here.
///   - New code should prefer `tracing::{info,warn,error,instrument}`,
///     which prints via this subscriber.
///   - We deliberately use `set_global_default` rather than `try_init` here.
///     `try_init` would also install `tracing_log::LogTracer` (via the
///     `tracing-log` default feature on `tracing-subscriber`), which calls
///     `log::set_logger` — and `tauri-plugin-log` then aborts the whole app
///     at startup with `PluginInitialization("log", "attempted to set a
///     logger after the logging system was already initialized")`. Until we
///     migrate to a single unified pipeline, the two stay independent:
///     `log` → tauri-plugin-log (file + console), `tracing` → stderr here.
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_env("MAGPIE_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("magpie_lib=info,warn"));

    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_target(true)
        .with_writer(std::io::stderr)
        .finish();

    // Ignore the error: a second `run()` call (tests, hot-reload) just keeps
    // the existing subscriber. Crucially, this does NOT install LogTracer,
    // so `tauri-plugin-log` is still free to set itself as the `log` global.
    let _ = tracing::subscriber::set_global_default(subscriber);
}

pub fn run() {
    init_tracing();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init());

    // Updater + process are desktop-only. The frontend invokes update
    // operations via our own `magpie_updater_*` commands (NOT
    // `@tauri-apps/plugin-updater`'s JS API), because the channel choice —
    // stable vs. beta — happens in Rust per call. The plugin's global
    // Builder does not accept endpoints (only tauri.conf.json does), but
    // the per-call `UpdaterBuilder` reached through
    // `UpdaterExt::updater_builder()` does. Our commands always call
    // `.endpoints(...)` based on `UserSettings.update_channel`, so the
    // value in tauri.conf.json is just a fallback that's never read.
    //
    // The plugin still has to be registered: its setup owns the pubkey
    // used to verify the .tar.gz signature, and `updater_builder()` reads
    // the plugin's `UpdaterState` for that pubkey.
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }

    let run_result = builder
        .manage(Arc::new(AppState::new()))
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::toggle_recording,
            commands::cancel_recording,
            commands::get_app_state,
            commands::get_available_models,
            commands::get_downloaded_models,
            commands::download_model,
            commands::cancel_download,
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
            commands::update_global_shortcut,
            commands::restart_app,
            commands::get_vocabulary,
            commands::add_vocabulary_entry,
            commands::remove_vocabulary_entry,
            commands::clear_vocabulary,
            commands::get_launch_at_login_status,
            commands::open_login_items_settings,
            // Styles + Profiles + Frontmost App + Running Apps (per-app profiles feature)
            commands::get_styles,
            commands::add_style,
            commands::update_style,
            commands::delete_style,
            commands::duplicate_style,
            commands::reset_style_to_default,
            commands::preview_style,
            commands::validate_transform,
            commands::get_profiles,
            commands::add_profile,
            commands::update_profile,
            commands::delete_profile,
            commands::duplicate_profile,
            commands::set_profile_enabled,
            commands::reset_built_in_presets,
            commands::get_frontmost_app,
            commands::get_running_apps,
            // Transcript history (local searchable log + re-paste)
            commands::get_transcription_history,
            commands::clear_transcription_history,
            commands::copy_history_entry_to_clipboard,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            commands::magpie_updater_check,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            commands::magpie_updater_install,
        ])
        .on_window_event(window_event_handler)
        .setup(|app| {
            // Tauri's setup closure runs inside tao's `did_finish_launching`,
            // which is an `extern "C"` Objective-C callback. Any panic here
            // would unwind across the C ABI boundary and abort the process
            // with "panic in a function that cannot unwind". We wrap the
            // entire setup body in catch_unwind to convert panics into errors.
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| setup::setup_app(app)));
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
                    write_startup_breadcrumb(&err_msg);
                    Err(err_msg.into())
                }
            }
        })
        .run(tauri::generate_context!());

    match run_result {
        Ok(()) => {}
        Err(e) => {
            // `tauri::Error` from `.run()` covers failures the builder itself
            // raises (plugin init, context generation, runtime crashes). The
            // setup-closure panic path above is separate. We log to the
            // breadcrumb file before propagating because tauri-plugin-log's
            // file writer may never have flushed if the failure was during
            // plugin init.
            let err_msg = format!("Error while running Magpie: {e}");
            eprintln!("{err_msg}");
            write_startup_breadcrumb(&err_msg);
            // Preserve the previous abort-on-failure behavior: this is a
            // fatal startup error and there is no meaningful fallback.
            panic!("{err_msg}");
        }
    }
}

/// Window-event handler: hides the main popover on focus-loss (unless
/// in setup or one-shot-suppressed) and prevents close from quitting
/// the app (just hide).
fn window_event_handler(window: &tauri::Window, event: &tauri::WindowEvent) {
    use tauri::Manager;
    match window.label() {
        "main" => match event {
            tauri::WindowEvent::Focused(false) => {
                use std::sync::atomic::Ordering;
                let state = window.app_handle().state::<Arc<AppState>>();

                // During setup the window shouldn't auto-hide on blur
                let in_setup = !state::lock_or_recover(&state.settings).setup_complete;

                // One-shot suppression (e.g. opening System Preferences)
                let suppressed = state.suppress_hide.swap(false, Ordering::SeqCst);

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
        "history" => match event {
            tauri::WindowEvent::Focused(false) => {
                let _ = window.hide();
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
        },
        _ => {}
    }
}
