//! Hotkey-monitor Tauri commands.
//!
//! The Fn key tap can be torn down and re-spun by either the user
//! (changing activation mode in Settings) or the watchdog in `lib.rs`
//! (detecting that macOS disabled the tap). [`restart_fn_key_monitor_inner`]
//! is the Rust-callable form that both code paths share; the Tauri
//! command variants are thin wrappers.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::hotkey;
use crate::state::{lock_or_recover, AppState};

/// Rust-callable helper that stops any running Fn key monitor and starts a
/// fresh one. Factored out so the watchdog in `lib.rs` can call it without
/// going through the Tauri command machinery (which requires `State<'_, _>`).
pub fn restart_fn_key_monitor_inner(
    app: &AppHandle,
    state: &Arc<AppState>,
) -> Result<bool, String> {
    // Stop the existing monitor before starting a new one
    if let Some(handle) = lock_or_recover(&state.fn_key_monitor).take() {
        handle.stop();
    }

    let activation_mode = lock_or_recover(&state.settings).activation_mode.clone();

    // Clone the recording command sender for the new monitor
    let tx = match lock_or_recover(&state.recording_tx).as_ref() {
        Some(tx) => tx.clone(),
        None => {
            return Err("Cannot restart fn key monitor: recording channel not initialized".into());
        }
    };

    let (new_handle, tap_ok) = hotkey::start_fn_key_monitor(app.clone(), tx, activation_mode);
    *lock_or_recover(&state.fn_key_monitor) = Some(new_handle);

    if !tap_ok {
        log::warn!("restart_fn_key_monitor: CGEventTap creation failed");
    }

    Ok(tap_ok)
}

#[tauri::command]
pub fn restart_fn_key_monitor(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    restart_fn_key_monitor_inner(&app, state.inner())
}

#[tauri::command]
pub fn get_fn_key_monitor_status(state: State<'_, Arc<AppState>>) -> bool {
    lock_or_recover(&state.fn_key_monitor)
        .as_ref()
        .map(|h| h.is_active())
        .unwrap_or(false)
}
