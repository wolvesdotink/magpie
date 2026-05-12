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
mod hotkey;
#[cfg(target_os = "macos")]
mod launch_at_login;
mod model_loading;
mod models;
mod output;
mod overlay;
mod permissions;
mod recording;
mod settings;
mod setup;
mod state;
mod transcription;
mod tray;
mod vocabulary;

use std::sync::Arc;

use crate::state::AppState;

// Re-export the public load helpers used by `commands::*` via `crate::*` paths,
// so the call sites stay short.
pub use model_loading::{
    flush_pending_reload, load_with_self_test, reload_backend_after_backfill_public,
};

/// Default global shortcut used when the user has not configured a custom one.
pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+Space";

/// Install a `tracing` subscriber for new structured-logging code. Filter
/// is taken from `MAGPIE_LOG` if set, else `RUST_LOG`, else `info` for the
/// app crate and `warn` for everything else.
///
/// Notes on coexistence with `log`:
///   - Existing `log::info!`/`debug!` etc. calls continue to flow through
///     `tauri-plugin-log` (file + console). They do NOT show up here.
///   - New code should prefer `tracing::{info,warn,error,instrument}`,
///     which prints via this subscriber.
///   - A future change can call `tracing_log::LogTracer::init()` to
///     forward `log` records into `tracing`, at which point `tauri-plugin-log`
///     would be dropped or reconfigured. Doing both is fine; doing it now
///     would silently mute `tauri-plugin-log`.
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_env("MAGPIE_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("magpie_lib=info,warn"));

    // try_init so a second `run()` call (e.g. tests, or hot-reload) does not
    // panic on "global default subscriber already set".
    let _ = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_target(true)
        .with_writer(std::io::stderr)
        .try_init();
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

    // Updater + process are desktop-only. The frontend invokes these through
    // @tauri-apps/plugin-updater and @tauri-apps/plugin-process; we only need
    // to register them on the Rust side here. Updates are signed/verified
    // against the public key in tauri.conf.json (plugins.updater.pubkey) and
    // fetched from the endpoint picked here based on the user's channel.
    //
    // We read UserSettings off disk to decide between the stable and beta
    // endpoints — AppState isn't `manage`d yet at this point in the
    // builder. `UserSettings::load()` is cheap (JSON parse, file-cached by
    // the OS) and AppState::new() will read the same file again moments
    // later; the duplicate read isn't worth optimizing away.
    //
    // Toggling the channel in Settings requires an app relaunch because
    // the Tauri JS plugin's check() does not accept an endpoints override
    // (CheckOptions only exposes headers/timeout/proxy/target/allowDowngrades).
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use crate::settings::{UpdateChannel, UserSettings};

        let endpoint_url = match UserSettings::load().update_channel {
            UpdateChannel::Beta => {
                "https://github.com/wolvesdotink/magpie/releases/download/beta-channel/latest.json"
            }
            UpdateChannel::Stable => {
                "https://github.com/wolvesdotink/magpie/releases/latest/download/latest.json"
            }
        };
        let endpoint: tauri::Url = endpoint_url
            .parse()
            .expect("hardcoded updater endpoint URL is valid");

        builder = builder
            .plugin(
                tauri_plugin_updater::Builder::new()
                    .endpoints(vec![endpoint])
                    .expect("setting hardcoded updater endpoint cannot fail")
                    .build(),
            )
            .plugin(tauri_plugin_process::init());
    }

    builder
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
            commands::repair_active_model,
            commands::get_launch_at_login_status,
            commands::open_login_items_settings,
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
                    Err(err_msg.into())
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Error while running Magpie");
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
        _ => {}
    }
}
