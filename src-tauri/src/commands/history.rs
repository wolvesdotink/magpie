//! Transcription-history Tauri commands.
//!
//! The on-disk ring lives in `AppState::history`. Pushes happen in the
//! recording pipeline (see `commands/recording.rs`); these commands cover
//! everything the History window needs: read, clear, and copy-to-clipboard.

use std::sync::Arc;

use tauri::State;

use crate::history::HistoryEntry;
use crate::output::clipboard::set_clipboard_text;
use crate::state::{lock_or_recover, AppState};

/// All history entries, newest first.
#[tauri::command]
pub fn get_transcription_history(state: State<'_, Arc<AppState>>) -> Vec<HistoryEntry> {
    lock_or_recover(&state.history).all()
}

/// Wipe the on-disk history.
#[tauri::command]
pub fn clear_transcription_history(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut hist = lock_or_recover(&state.history);
    hist.clear();
    hist.save().map_err(|e| e.to_string())
}

/// Copy a history entry's text onto the system clipboard. The History
/// window calls this and then hides itself so the user can paste into
/// whatever app they actually want.
#[tauri::command]
pub fn copy_history_entry_to_clipboard(text: String) -> Result<(), String> {
    set_clipboard_text(&text).map_err(|e| e.to_string())
}
