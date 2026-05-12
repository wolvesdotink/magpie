//! Settings, global-shortcut, and launch-at-login Tauri commands.
//!
//! Everything the Settings window directly mutates lives here:
//!   - get_settings / update_settings (the catch-all preference round trip)
//!   - update_global_shortcut (re-bind the dictation hotkey)
//!   - get_launch_at_login_status / open_login_items_settings (macOS
//!     SMAppService surface, no-op stubs on other platforms)
//!
//! Model selections (whisper + correction) are NOT here even though they
//! also touch settings; they live in commands/models.rs because the save
//! is a side effect of the select_* download/select flow, not a generic
//! preference update.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::state::{lock_or_recover, AppState};

/// Get the current UserSettings.
#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> crate::settings::UserSettings {
    lock_or_recover(&state.settings).clone()
}

/// Replace the current UserSettings.
///
/// Special handling: a `None` for `selected_model` or
/// `selected_correction_model` in the incoming payload means "I don't know
/// / I didn't touch this", not "clear the selection". Those fields are
/// owned by the model-management commands, not by generic preference
/// saves; we preserve the backend's value to avoid Settings save
/// clobbering an in-flight download.
#[tauri::command]
pub fn update_settings(
    state: State<'_, Arc<AppState>>,
    mut settings: crate::settings::UserSettings,
) {
    let mut current = lock_or_recover(&state.settings);
    if settings.selected_model.is_none() {
        settings.selected_model = current.selected_model.clone();
    }
    if settings.selected_correction_model.is_none() {
        settings.selected_correction_model = current.selected_correction_model.clone();
    }
    let auto_start_changed = current.auto_start != settings.auto_start;
    let new_auto_start = settings.auto_start;
    *current = settings;
    if let Err(e) = current.save() {
        log::error!("Failed to save settings: {}", e);
    }
    log::info!(
        "Settings updated; selected_model={:?}, selected_correction_model={:?}",
        current.selected_model,
        current.selected_correction_model
    );
    // Drop the settings lock before calling into SMAppService — the
    // register/unregister call can take a noticeable moment and may
    // surface a system notification on the main thread.
    drop(current);

    #[cfg(target_os = "macos")]
    if auto_start_changed {
        match crate::launch_at_login::set_enabled(new_auto_start) {
            Ok(status) => log::info!(
                "launch-at-login set to {}: status={:?}",
                new_auto_start,
                status
            ),
            Err(e) => log::error!(
                "launch-at-login set_enabled({}) failed: {}",
                new_auto_start,
                e
            ),
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (auto_start_changed, new_auto_start);
}

/// Re-register the global keyboard shortcut. `shortcut = None` reverts to the
/// built-in default. Returns an error if the new combination fails to parse
/// or cannot be registered (e.g. already claimed by another app).
#[tauri::command]
pub fn update_global_shortcut(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    shortcut: Option<String>,
) -> Result<(), String> {
    use crate::recording::RecordingCommand;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let new_str = shortcut
        .clone()
        .unwrap_or_else(|| crate::DEFAULT_SHORTCUT.to_string());
    let new_shortcut: Shortcut = new_str
        .parse()
        .map_err(|e| format!("Invalid shortcut '{}': {}", new_str, e))?;

    // Best-effort unregister of the previously-registered shortcut so we don't
    // leak a stale binding when the user switches combinations. Errors are
    // ignored (it might not be registered, or the binding might be stale).
    let old_str_opt = lock_or_recover(&state.current_shortcut).clone();
    if let Some(old_str) = old_str_opt {
        if let Ok(old) = old_str.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(old);
        }
    }

    // Clone the recording-command sender so the callback can dispatch toggles.
    let tx = match lock_or_recover(&state.recording_tx).as_ref() {
        Some(tx) => tx.clone(),
        None => return Err("Recording channel not initialized".into()),
    };

    app.global_shortcut()
        .on_shortcut(new_shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(e) = tx.send(RecordingCommand::Toggle) {
                    log::error!("Failed to send toggle command: {}", e);
                }
            }
        })
        .map_err(|e| format!("Failed to register shortcut: {}", e))?;

    // Persist the new value (or `None` for "use default") and update the
    // tracked string so the next re-register knows what to unregister.
    {
        let mut settings = lock_or_recover(&state.settings);
        settings.custom_shortcut = shortcut;
        if let Err(e) = settings.save() {
            log::error!("Failed to save settings after shortcut change: {}", e);
        }
    }
    *lock_or_recover(&state.current_shortcut) = Some(new_str.clone());

    log::info!("Global shortcut updated: {}", new_str);
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub fn get_launch_at_login_status() -> crate::launch_at_login::LaunchAtLoginStatus {
    crate::launch_at_login::status()
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub fn get_launch_at_login_status() -> &'static str {
    "notRegistered"
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub fn open_login_items_settings() {
    crate::launch_at_login::open_login_items_settings();
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub fn open_login_items_settings() {}
