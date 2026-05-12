//! App-startup wiring.
//!
//! Called from [`crate::run`] inside Tauri's `setup` closure. Everything
//! that needs to happen before the first user interaction lives here:
//!   - Dock-icon policy (menu-bar-only on macOS)
//!   - Tray + overlay window setup
//!   - Settings-window transparency
//!   - Stale-download cleanup
//!   - Whisper + correction model auto-load (delegated to model_loading)
//!   - launch-at-login reconciliation
//!   - First-launch window-show logic
//!   - Recording-command mpsc channel + consumer task
//!   - Fn key monitor + watchdog
//!   - Global shortcut registration

use std::sync::Arc;
use std::time::Duration;

use tauri::Manager;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::commands;
use crate::hotkey;
use crate::model_loading::{try_load_last_correction_model, try_load_last_model};
use crate::overlay;
use crate::permissions;
use crate::recording::RecordingCommand;
use crate::state::{lock_or_recover, AppState};
use crate::tray;
use crate::{models, DEFAULT_SHORTCUT};

#[cfg(target_os = "macos")]
use crate::launch_at_login;

/// The actual setup logic, extracted so it can be wrapped in catch_unwind
/// by the Tauri `setup` closure.
pub(crate) fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
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
    overlay::setup_overlay(app.handle());

    // Force settings window to be fully transparent so the
    // CSS outer padding shows through as empty space.
    if let Some(settings_win) = app.get_webview_window("settings") {
        let _ = settings_win.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
    } else {
        log::warn!("Settings window not found during setup");
    }

    cleanup_stale_downloads();

    let state = app.state::<Arc<AppState>>();
    try_load_last_model(&state, app.handle());
    try_load_last_correction_model(&state);

    // Reconcile launch-at-login: settings.json is authoritative for intent,
    // but System Settings → Login Items is authoritative for OS state. If
    // the user toggled it off via System Settings while the app was closed,
    // sync that back so the UI reflects reality.
    #[cfg(target_os = "macos")]
    {
        let stored = lock_or_recover(&state.settings).auto_start;
        let actual = launch_at_login::status();
        let actual_enabled = matches!(
            actual,
            launch_at_login::LaunchAtLoginStatus::Enabled
                | launch_at_login::LaunchAtLoginStatus::RequiresApproval
        );
        if stored != actual_enabled {
            log::info!(
                "launch-at-login drift: settings={}, actual={:?} — syncing setting to OS state",
                stored,
                actual
            );
            let mut s = lock_or_recover(&state.settings);
            s.auto_start = actual_enabled;
            let _ = s.save();
        }
    }

    // Show the main window on startup if anything blocks normal operation:
    // no model loaded, or any of the three required permissions revoked.
    // App.vue routes to the permissions guide when any perm is missing, so
    // surfacing the window is enough — without this, a returning user who
    // revoked a permission would press Fn and get no signal.
    {
        let has_model = lock_or_recover(&state.backend).is_some();
        let perms = commands::check_permissions();
        let missing_perms = !perms.microphone || !perms.accessibility || !perms.input_monitoring;
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
    *lock_or_recover(&state.recording_tx) = Some(tx.clone());

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
        if input_monitoring_granted {
            "granted"
        } else {
            "NOT granted — user must enable in System Settings"
        }
    );

    // Start Fn key monitor — pass channel sender directly (no Tauri events)
    let app_handle = app.handle().clone();
    let activation_mode = lock_or_recover(&state.settings).activation_mode.clone();
    let hotkey_tx = tx.clone();
    let (monitor_handle, monitor_ok) =
        hotkey::start_fn_key_monitor(app_handle.clone(), hotkey_tx, activation_mode);
    if !monitor_ok {
        log::warn!(
            "Fn key monitor failed to start — Input Monitoring permission likely missing. \
             Banner will direct the user to System Settings."
        );
    }
    *lock_or_recover(&state.fn_key_monitor) = Some(monitor_handle);

    // Fn monitor watchdog. The tap callback tries to re-enable itself when
    // macOS disables the tap (see hotkey.rs). If that fast-path fails (e.g.
    // the tap was disabled before the mach port was stored, or re-enable
    // didn't take), the callback sets a `needs_restart` flag. This watchdog
    // polls the flag every 10 s and tears down + restarts the monitor when
    // set. It also gives us a recovery path for zombie taps that stop
    // delivering events without triggering a disable event.
    let watchdog_app = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        // Initial grace period so the monitor can finish spinning up before
        // the first health check.
        sleep(Duration::from_secs(10)).await;

        loop {
            sleep(Duration::from_secs(10)).await;

            let state = watchdog_app.state::<Arc<AppState>>();
            let needs_restart = lock_or_recover(&state.fn_key_monitor)
                .as_ref()
                .map(|h| h.needs_restart())
                .unwrap_or(false);

            if needs_restart {
                log::info!("Fn key monitor watchdog: needs_restart flag set — restarting");
                match commands::restart_fn_key_monitor_inner(&watchdog_app, state.inner()) {
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
                        if let Err(e) =
                            commands::start_recording(consumer_app.clone(), state.clone()).await
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
                        if let Err(e) =
                            commands::stop_recording(consumer_app.clone(), state.clone()).await
                        {
                            log::error!("Failed to stop recording: {}", e);
                        }
                    } else {
                        log::debug!("Ignoring stop command: not recording");
                    }
                }
                RecordingCommand::Toggle => {
                    if state.is_recording() {
                        if let Err(e) =
                            commands::stop_recording(consumer_app.clone(), state.clone()).await
                        {
                            log::error!("Failed to stop recording: {}", e);
                        }
                    } else if !state.is_processing() {
                        if let Err(e) =
                            commands::start_recording(consumer_app.clone(), state.clone()).await
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

/// Remove leftover `.downloading` temp files from interrupted downloads.
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

/// Register the global keyboard shortcut on startup, reading the user's
/// custom value from settings if present, otherwise using
/// [`crate::DEFAULT_SHORTCUT`]. Records the chosen shortcut string in
/// `AppState::current_shortcut` so that `update_global_shortcut` knows
/// what to unregister before binding a new one.
fn register_global_shortcut(
    app: &tauri::App,
    tx: mpsc::UnboundedSender<RecordingCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let state = app.state::<Arc<AppState>>();
    let custom = lock_or_recover(&state.settings).custom_shortcut.clone();
    let shortcut_str = custom.unwrap_or_else(|| DEFAULT_SHORTCUT.to_string());

    let shortcut: Shortcut = match shortcut_str.parse() {
        Ok(s) => s,
        Err(e) => {
            log::warn!(
                "Saved custom shortcut '{}' failed to parse ({}); falling back to default",
                shortcut_str,
                e
            );
            DEFAULT_SHORTCUT.parse()?
        }
    };

    log::info!("Registering global shortcut: {}", shortcut_str);

    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(e) = tx.send(RecordingCommand::Toggle) {
                    log::error!("Failed to send toggle command: {}", e);
                }
            }
        })?;

    *lock_or_recover(&state.current_shortcut) = Some(shortcut_str);

    Ok(())
}
