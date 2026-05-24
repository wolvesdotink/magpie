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
    let has_model = crate::model_loading::has_usable_model(&state);
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
    let has_model = crate::model_loading::has_usable_model(state);
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

/// Total physical RAM in bytes. The model picker uses this to size its
/// memory-cost warnings to the user's machine (e.g. flag a heavy model on an
/// 8 GB Mac). Returns 0 when the value can't be determined; the frontend
/// treats 0 as "unknown" and skips the warning.
#[tauri::command]
pub fn get_system_memory() -> u64 {
    system_memory_bytes()
}

/// Read `NSProcessInfo.physicalMemory` — total installed RAM in bytes. Uses
/// the Foundation framework already linked via `cocoa`/`objc`, so no extra
/// dependency.
#[cfg(target_os = "macos")]
fn system_memory_bytes() -> u64 {
    use objc::{class, msg_send, sel, sel_impl};
    // SAFETY: standard Foundation messaging. `processInfo` is a shared
    // singleton; `physicalMemory` returns an NSUInteger (u64) and mutates
    // nothing. We guard against a null singleton before sending the second
    // message.
    unsafe {
        let process_info: *mut objc::runtime::Object =
            msg_send![class!(NSProcessInfo), processInfo];
        if process_info.is_null() {
            return 0;
        }
        msg_send![process_info, physicalMemory]
    }
}

#[cfg(not(target_os = "macos"))]
fn system_memory_bytes() -> u64 {
    0
}
