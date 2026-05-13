//! Tauri commands for the reusable Styles library.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::command_error::CommandError;
use crate::events::{self, event_names};
use crate::state::{lock_or_recover, AppState};
use crate::styles::{Style, StylesError, TextTransform, ValidationResult};
use crate::transcription::{custom_rules, postprocess};

#[tauri::command]
pub fn get_styles(state: State<'_, Arc<AppState>>) -> Vec<Style> {
    lock_or_recover(&state.styles).styles.clone()
}

#[tauri::command]
pub fn add_style(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    style: Style,
) -> Result<Style, CommandError> {
    let added = {
        let mut store = lock_or_recover(&state.styles);
        let added = store.add(style);
        store.save()?;
        added
    };
    events::emit_event(&app, event_names::STYLES_CHANGED, ());
    Ok(added)
}

#[tauri::command]
pub fn update_style(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    style: Style,
) -> Result<Style, CommandError> {
    let updated = {
        let mut store = lock_or_recover(&state.styles);
        let updated = store.update(&id, style)?;
        store.save()?;
        updated
    };
    events::emit_event(&app, event_names::STYLES_CHANGED, ());
    Ok(updated)
}

#[tauri::command]
pub fn delete_style(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), CommandError> {
    // Block deletion if any profile references this style.
    let blocking_profiles: Vec<String> = {
        let profiles = lock_or_recover(&state.profiles);
        profiles
            .profiles
            .iter()
            .filter(|p| p.style_id == id)
            .map(|p| p.display_name.clone())
            .collect()
    };
    if !blocking_profiles.is_empty() {
        return Err(StylesError::StyleInUse {
            profile_names: blocking_profiles,
        }
        .into());
    }
    {
        let mut store = lock_or_recover(&state.styles);
        store.delete(&id)?;
        store.save()?;
    }
    events::emit_event(&app, event_names::STYLES_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn duplicate_style(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Style, CommandError> {
    let dup = {
        let mut store = lock_or_recover(&state.styles);
        let dup = store.duplicate(&id)?;
        store.save()?;
        dup
    };
    events::emit_event(&app, event_names::STYLES_CHANGED, ());
    Ok(dup)
}

#[tauri::command]
pub fn reset_style_to_default(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Style, CommandError> {
    let updated = {
        let mut store = lock_or_recover(&state.styles);
        let updated = store.reset_to_default(&id)?;
        store.save()?;
        updated
    };
    events::emit_event(&app, event_names::STYLES_CHANGED, ());
    Ok(updated)
}

/// Run the postprocess pipeline on `sample_text` using the given style.
/// Used by the Style editor's live preview pane. Does NOT mutate any state.
#[tauri::command]
pub fn preview_style(style: Style, sample_text: String) -> Result<String, CommandError> {
    let compiled = custom_rules::compile_all(&style.custom_rules).map_err(|e| {
        CommandError::InvalidArgument {
            message: format!("custom rules: {e}"),
        }
    })?;
    let remove_fillers = style.filler_override.unwrap_or(true);
    // Use a sensible default filler list for the preview so users see filler
    // removal behaving as it would during a real recording.
    let fillers = vec![
        "um".to_string(),
        "uh".to_string(),
        "hmm".to_string(),
        "mm".to_string(),
        "ah".to_string(),
        "er".to_string(),
    ];
    Ok(postprocess::postprocess(
        &sample_text,
        &fillers,
        remove_fillers,
        &[],
        &style.formatting,
        &compiled,
    ))
}

#[tauri::command]
pub fn validate_transform(transform: TextTransform) -> ValidationResult {
    match custom_rules::validate(&transform) {
        Ok(()) => ValidationResult::ok(),
        Err(msg) => ValidationResult::err(msg),
    }
}
