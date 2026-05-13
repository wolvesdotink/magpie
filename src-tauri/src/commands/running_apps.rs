//! Running-apps Tauri command.
//!
//! Powers the Settings → App Profiles "Choose app…" picker so the user can
//! select a target app from the list of currently running applications
//! (instead of relying on frontmost-app detection, which always reports
//! Magpie because the Settings window has focus at click time).

use crate::running_apps::{self, RunningApp};

#[tauri::command]
pub fn get_running_apps() -> Vec<RunningApp> {
    running_apps::list()
}
