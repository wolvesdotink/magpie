//! Channel-aware in-app updater commands.
//!
//! `tauri_plugin_updater`'s plugin-level `Builder` does not accept
//! endpoints (only `tauri.conf.json` does), so we cannot register a
//! channel-aware endpoint at startup. Instead, the per-call
//! `UpdaterBuilder` accessed via `UpdaterExt::updater_builder()` DOES
//! accept `.endpoints(...)`, and we use that here — reading the user's
//! channel from `UserSettings` on each call.
//!
//! Flow:
//!   1. JS calls `magpie_updater_check`. Rust builds an `Updater` with the
//!      channel-appropriate endpoint, runs `.check()`, stores the resulting
//!      `Update` handle in `AppState.pending_update`, and returns metadata
//!      to the frontend.
//!   2. JS shows the "Update available" UI. When the user confirms, JS
//!      calls `magpie_updater_install`, which takes the stored `Update`
//!      and runs `.download_and_install(...)` — emitting per-chunk
//!      progress as `magpie://updater-progress` events.
//!
//! Signature verification continues to happen inside `Update::download`
//! using the embedded pubkey from `tauri.conf.json::plugins.updater.pubkey`.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::UpdaterExt;

use crate::settings::UpdateChannel;
use crate::state::{lock_or_recover, AppState};

const STABLE_ENDPOINT: &str =
    "https://github.com/wolvesdotink/magpie/releases/latest/download/latest.json";
const BETA_ENDPOINT: &str =
    "https://github.com/wolvesdotink/magpie/releases/download/beta-channel/latest.json";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterCheckResult {
    pub version: String,
    pub current_version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    chunk_length: usize,
    content_length: Option<u64>,
}

#[tauri::command]
pub async fn magpie_updater_check(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<UpdaterCheckResult>, String> {
    let channel = lock_or_recover(&state.settings).update_channel;
    let endpoint_url = match channel {
        UpdateChannel::Beta => BETA_ENDPOINT,
        UpdateChannel::Stable => STABLE_ENDPOINT,
    };
    let endpoint: tauri::Url = endpoint_url
        .parse()
        .map_err(|e| format!("parse endpoint: {e}"))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("set endpoints: {e}"))?
        .build()
        .map_err(|e| format!("build updater: {e}"))?;

    let maybe_update = updater
        .check()
        .await
        .map_err(|e| format!("check update: {e}"))?;

    match maybe_update {
        Some(update) => {
            let result = UpdaterCheckResult {
                version: update.version.clone(),
                current_version: update.current_version.clone(),
                body: update.body.clone(),
                date: update.date.map(|d| d.to_string()),
            };
            *lock_or_recover(&state.pending_update) = Some(update);
            Ok(Some(result))
        }
        None => {
            *lock_or_recover(&state.pending_update) = None;
            Ok(None)
        }
    }
}

#[tauri::command]
pub async fn magpie_updater_install(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Take ownership of the pending Update — download_and_install consumes
    // it. If install fails, the next call has to start with a fresh check
    // (which is fine: it's cheap, and re-stashes a new Update).
    let update = lock_or_recover(&state.pending_update)
        .take()
        .ok_or_else(|| "no pending update; call magpie_updater_check first".to_string())?;

    let app_for_progress = app.clone();
    let app_for_finish = app.clone();

    update
        .download_and_install(
            move |chunk_length, content_length| {
                let _ = app_for_progress.emit(
                    "magpie://updater-progress",
                    ProgressPayload {
                        chunk_length,
                        content_length,
                    },
                );
            },
            move || {
                let _ = app_for_finish.emit("magpie://updater-finished", ());
            },
        )
        .await
        .map_err(|e| format!("download and install: {e}"))?;

    Ok(())
}
