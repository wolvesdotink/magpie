//! Permissions Tauri commands.
//!
//! Thin wrappers around `crate::permissions::*` that the frontend's
//! `PermissionsGuide` view invokes. The "open settings" variants flip
//! `state.suppress_hide` so that the next window-focus-lost event (caused
//! by System Preferences taking focus) doesn't hide the main popover —
//! otherwise the user would lose track of where they were.

use std::sync::Arc;

use tauri::State;

use crate::command_error::CommandError;
use crate::events::PermissionsPayload;
use crate::state::AppState;

#[tauri::command]
pub fn check_permissions() -> PermissionsPayload {
    let mic_status = crate::permissions::microphone_authorization_status();
    PermissionsPayload {
        microphone: mic_status == crate::permissions::MicrophoneAuthStatus::Authorized,
        accessibility: crate::permissions::is_accessibility_trusted(),
        input_monitoring: crate::permissions::is_input_monitoring_trusted(),
    }
}

#[tauri::command]
pub fn request_microphone_permission() -> bool {
    crate::permissions::request_microphone_access()
}

#[tauri::command]
pub fn open_microphone_settings(state: State<'_, Arc<AppState>>) -> Result<(), CommandError> {
    state
        .suppress_hide
        .store(true, std::sync::atomic::Ordering::SeqCst);
    crate::permissions::open_microphone_settings().map_err(CommandError::from)
}

#[tauri::command]
pub fn open_accessibility_settings(state: State<'_, Arc<AppState>>) -> Result<(), CommandError> {
    state
        .suppress_hide
        .store(true, std::sync::atomic::Ordering::SeqCst);
    crate::permissions::open_accessibility_settings().map_err(CommandError::from)
}

/// Trigger the Input Monitoring TCC prompt. On first call this prompts the
/// user and adds the app to System Settings → Privacy & Security → Input
/// Monitoring so the toggle becomes available. After the user grants it,
/// the Fn key monitor must be restarted to pick up the new permission.
#[tauri::command]
pub fn request_input_monitoring_permission() -> bool {
    crate::permissions::request_input_monitoring_access()
}

#[tauri::command]
pub fn open_input_monitoring_settings(state: State<'_, Arc<AppState>>) -> Result<(), CommandError> {
    state
        .suppress_hide
        .store(true, std::sync::atomic::Ordering::SeqCst);
    crate::permissions::open_input_monitoring_settings().map_err(CommandError::from)
}
