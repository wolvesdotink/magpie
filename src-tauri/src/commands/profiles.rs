//! Tauri commands for per-app profiles.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::command_error::CommandError;
use crate::events::{self, event_names};
use crate::profiles::AppProfile;
use crate::state::{lock_or_recover, AppState};

#[tauri::command]
pub fn get_profiles(state: State<'_, Arc<AppState>>) -> Vec<AppProfile> {
    lock_or_recover(&state.profiles).profiles.clone()
}

#[tauri::command]
pub fn add_profile(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    profile: AppProfile,
) -> Result<AppProfile, CommandError> {
    let added = {
        let mut store = lock_or_recover(&state.profiles);
        let added = store.add(profile);
        store.save()?;
        added
    };
    events::emit_event(&app, event_names::PROFILES_CHANGED, ());
    Ok(added)
}

#[tauri::command]
pub fn update_profile(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    profile: AppProfile,
) -> Result<AppProfile, CommandError> {
    let updated = {
        let mut store = lock_or_recover(&state.profiles);
        let updated = store.update(&id, profile)?;
        store.save()?;
        updated
    };
    events::emit_event(&app, event_names::PROFILES_CHANGED, ());
    Ok(updated)
}

#[tauri::command]
pub fn delete_profile(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), CommandError> {
    {
        let mut store = lock_or_recover(&state.profiles);
        store.delete(&id)?;
        store.save()?;
    }
    events::emit_event(&app, event_names::PROFILES_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn duplicate_profile(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<AppProfile, CommandError> {
    let dup = {
        let mut store = lock_or_recover(&state.profiles);
        let dup = store.duplicate(&id)?;
        store.save()?;
        dup
    };
    events::emit_event(&app, event_names::PROFILES_CHANGED, ());
    Ok(dup)
}

#[tauri::command]
pub fn set_profile_enabled(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    enabled: bool,
) -> Result<(), CommandError> {
    {
        let mut store = lock_or_recover(&state.profiles);
        store.set_enabled(&id, enabled)?;
        store.save()?;
    }
    events::emit_event(&app, event_names::PROFILES_CHANGED, ());
    Ok(())
}

/// Re-install any missing built-in styles and profiles atomically.
/// User-created entries are untouched.
#[tauri::command]
pub fn reset_built_in_presets(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), CommandError> {
    // Restore built-in styles first so any profile FKs resolve.
    {
        let mut styles = lock_or_recover(&state.styles);
        styles.ensure_builtins();
        styles.save()?;
    }
    {
        let mut profiles = lock_or_recover(&state.profiles);
        profiles.ensure_builtins();
        profiles.save()?;
    }
    events::emit_event(&app, event_names::STYLES_CHANGED, ());
    events::emit_event(&app, event_names::PROFILES_CHANGED, ());
    Ok(())
}
