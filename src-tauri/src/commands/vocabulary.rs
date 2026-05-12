//! Vocabulary Tauri commands.
//!
//! All vocabulary edits go through the on-disk JSON file via
//! `crate::vocabulary::Vocabulary`. The commands here are thin wrappers
//! around lock-and-mutate plus a `save()` call.

use std::sync::Arc;

use tauri::State;

use crate::state::{lock_or_recover, AppState};
use crate::vocabulary::{VocabularyEntry, VocabularySource};

#[tauri::command]
pub fn get_vocabulary(state: State<'_, Arc<AppState>>) -> Vec<VocabularyEntry> {
    lock_or_recover(&state.vocabulary).entries.clone()
}

#[tauri::command]
pub fn add_vocabulary_entry(
    state: State<'_, Arc<AppState>>,
    wrong: String,
    correct: String,
) -> Result<(), String> {
    let mut vocab = lock_or_recover(&state.vocabulary);
    vocab.add_or_update(&wrong, &correct, VocabularySource::Manual);
    vocab.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_vocabulary_entry(
    state: State<'_, Arc<AppState>>,
    wrong: String,
) -> Result<(), String> {
    let mut vocab = lock_or_recover(&state.vocabulary);
    vocab.remove(&wrong);
    vocab.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_vocabulary(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut vocab = lock_or_recover(&state.vocabulary);
    vocab.entries.clear();
    vocab.save().map_err(|e| e.to_string())
}
