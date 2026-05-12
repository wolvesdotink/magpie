//! Feature-flags Tauri commands.

use std::sync::Arc;

use tauri::State;

use crate::features::FeatureFlags;
use crate::state::{lock_or_recover, AppState};

/// Resolve the current set of feature flags from settings + env overrides.
/// Called once at app start by the frontend; the result is cached for the
/// session. Cheap to call — pure function over current state.
#[tauri::command]
pub fn get_feature_flags(state: State<'_, Arc<AppState>>) -> FeatureFlags {
    let settings = lock_or_recover(&state.settings);
    FeatureFlags::resolve(&settings)
}
