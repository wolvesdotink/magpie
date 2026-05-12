//! Application-lifecycle Tauri commands: state snapshot and restart.
//!
//! `get_app_state` (Tauri) and `get_app_state_payload` (Rust-internal)
//! both build the same shape — the Tauri-callable version takes a
//! `State<'_, _>` directly, the helper accepts an `&State` so other
//! command handlers can call it without borrowing twice. Both live here
//! so the model-management code can call `app::get_app_state_payload`
//! without round-tripping through `mod.rs`.

use std::sync::Arc;

use tauri::State;

use crate::events::AppStatePayload;
use crate::state::{lock_or_recover, AppState};

#[tauri::command]
pub fn get_app_state(state: State<'_, Arc<AppState>>) -> AppStatePayload {
    let has_model = lock_or_recover(&state.backend).is_some();
    let last_transcription = lock_or_recover(&state.last_transcription).clone();

    AppStatePayload {
        recording: state.is_recording(),
        processing: state.is_processing(),
        has_model,
        last_transcription,
    }
}

/// `pub(crate)` internal helper used by command handlers (e.g. the repair
/// flow) that already hold a `State` reference and want to re-emit the
/// current snapshot. Identical body to `get_app_state` — calls don't go
/// through the Tauri command machinery.
pub(crate) fn get_app_state_payload(state: &State<'_, Arc<AppState>>) -> AppStatePayload {
    let has_model = lock_or_recover(&state.backend).is_some();
    let last_transcription = lock_or_recover(&state.last_transcription).clone();

    AppStatePayload {
        recording: state.is_recording(),
        processing: state.is_processing(),
        has_model,
        last_transcription,
    }
}

/// Restart the app. Re-execs the current binary; on macOS this lets a freshly
/// granted Accessibility permission take effect when AX trust was cached as
/// `false` for the previous process lifetime.
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    app.restart();
}
