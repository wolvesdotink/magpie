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

use crate::command_error::CommandError;
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
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    mut settings: crate::settings::UserSettings,
) {
    // History-disabled state is computed BEFORE the clamp so that a
    // payload of `history_max_entries == 0` is still recognised as
    // "disabled" — the clamp would otherwise rewrite it to MIN_ENTRIES
    // and hide that signal. `!history_enabled` is the primary path; the
    // zero check is a defensive backstop for hand-edited settings.json.
    let new_disabled = !settings.history_enabled || settings.history_max_entries == 0;

    // Clamp `history_max_entries` server-side so a hand-crafted payload (or
    // a future UI bug) cannot push the on-disk ring outside its supported
    // size envelope. The clamp range lives next to the constants in
    // `crate::history`.
    settings.history_max_entries = settings.history_max_entries.clamp(
        crate::history::HISTORY_MIN_ENTRIES,
        crate::history::HISTORY_MAX_ENTRIES,
    );
    let new_history_cap = settings.history_max_entries as usize;

    let mut current = lock_or_recover(&state.settings);
    if settings.selected_model.is_none() {
        settings.selected_model = current.selected_model.clone();
    }
    if settings.selected_correction_model.is_none() {
        settings.selected_correction_model = current.selected_correction_model.clone();
    }
    let auto_start_changed = current.auto_start != settings.auto_start;
    let new_auto_start = settings.auto_start;
    let prior_disabled = !current.history_enabled || current.history_max_entries == 0;
    let disabled_state_changed = prior_disabled != new_disabled;
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

    // History side-effects: disabling clears the ring outright; otherwise a
    // lowered cap trims excess entries. Both paths emit HISTORY_ENTRY_ADDED
    // so the History window's existing listener refreshes.
    // settings (rank 1) is released above; history (rank 7.5) goes next.
    {
        let mut hist = lock_or_recover(&state.history);
        if new_disabled {
            if hist.len() > 0 {
                hist.clear();
                if let Err(e) = hist.save() {
                    log::warn!("Failed to save cleared history after disable: {}", e);
                }
                crate::events::emit_event(
                    &app,
                    crate::events::event_names::HISTORY_ENTRY_ADDED,
                    (),
                );
            }
        } else if hist.len() > new_history_cap {
            hist.truncate_to(new_history_cap);
            if let Err(e) = hist.save() {
                log::warn!("Failed to save trimmed history after cap change: {}", e);
            }
            crate::events::emit_event(&app, crate::events::event_names::HISTORY_ENTRY_ADDED, ());
        }
    }

    // Rebuild the tray menu when the disabled state flips so "History…"
    // appears/disappears live without a restart. Also ping the History
    // window so it re-renders into / out of the "disabled" message (the
    // clear/trim branches above only fire when there's something to
    // remove — re-enable from an empty ring would otherwise miss).
    if disabled_state_changed {
        crate::tray::rebuild_tray_menu(&app);
        crate::events::emit_event(&app, crate::events::event_names::HISTORY_ENTRY_ADDED, ());
    }

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
) -> Result<(), CommandError> {
    use crate::recording::RecordingCommand;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let new_str = shortcut
        .clone()
        .unwrap_or_else(|| crate::DEFAULT_SHORTCUT.to_string());
    let new_shortcut: Shortcut = new_str.parse().map_err(|e| CommandError::InvalidArgument {
        message: format!("Invalid shortcut '{}': {}", new_str, e),
    })?;

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
        None => return Err(CommandError::other("Recording channel not initialized")),
    };

    app.global_shortcut()
        .on_shortcut(new_shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(e) = tx.send(RecordingCommand::Toggle) {
                    log::error!("Failed to send toggle command: {}", e);
                }
            }
        })
        .map_err(|e| CommandError::other(format!("Failed to register shortcut: {}", e)))?;

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

/// Return the built-in default voice-commands instruction block. The settings
/// UI uses this as the textarea's default content and as the "Restore default"
/// target. The string is the pure body text \u{2014} the spacing/separator
/// between it and the base correction prompt is added by
/// `augment_prompt_with_commands` at composition time.
#[tauri::command]
pub fn get_default_voice_commands_prompt() -> &'static str {
    crate::correction::engine::VOICE_COMMANDS_INSTRUCTIONS
}
