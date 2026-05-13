//! Frontmost-app Tauri commands.
//!
//! The Settings UI's "Detect current app" button calls this synchronously to
//! prefill a profile's bundle_id + display_name when creating or editing a
//! profile.

use crate::frontmost_app::{self, FrontmostApp};

#[tauri::command]
pub fn get_frontmost_app() -> Option<FrontmostApp> {
    frontmost_app::detect()
}
