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

use semver::Version;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::settings::UpdateChannel;
use crate::state::{lock_or_recover, AppState};

const STABLE_ENDPOINT: &str =
    "https://github.com/wolvesdotink/magpie/releases/latest/download/latest.json";
const BETA_ENDPOINT: &str =
    "https://raw.githubusercontent.com/wolvesdotink/magpie/manifests/beta.json";

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

async fn check_endpoint(app: &AppHandle, endpoint_url: &str) -> Result<Option<Update>, String> {
    let endpoint: tauri::Url = endpoint_url
        .parse()
        .map_err(|e| format!("parse endpoint: {e}"))?;
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("set endpoints: {e}"))?
        .build()
        .map_err(|e| format!("build updater: {e}"))?;
    updater
        .check()
        .await
        .map_err(|e| format!("check update: {e}"))
}

// Returns whichever side names a newer SemVer version. Generic over the
// payload (`T`) so we can unit-test the version logic without constructing a
// real `tauri_plugin_updater::Update`. On ties or unparseable versions, prefer
// `a` — at call sites we pass beta as `a`, which keeps the beta build for
// equal versions and falls back gracefully if a manifest carries a non-SemVer
// string.
fn pick_newer<T>(a: Option<(String, T)>, b: Option<(String, T)>) -> Option<T> {
    match (a, b) {
        (Some((av, ax)), Some((bv, bx))) => {
            let take_b = matches!(
                (Version::parse(&av), Version::parse(&bv)),
                (Ok(a), Ok(b)) if b > a
            );
            if take_b {
                Some(bx)
            } else {
                Some(ax)
            }
        }
        (Some((_, ax)), None) => Some(ax),
        (None, Some((_, bx))) => Some(bx),
        (None, None) => None,
    }
}

#[tauri::command]
pub async fn magpie_updater_check(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<UpdaterCheckResult>, String> {
    let channel = lock_or_recover(&state.settings).update_channel;

    let maybe_update = match channel {
        UpdateChannel::Stable => check_endpoint(&app, STABLE_ENDPOINT).await?,
        UpdateChannel::Beta => {
            // Fetch both manifests; on the Beta channel a newer Stable should
            // also be offered (e.g. when a stable release jumps past the
            // latest beta). Either endpoint may legitimately fail (network,
            // 404 before first beta has been published, etc.) — only surface
            // an error if both fail.
            let (beta_res, stable_res) = tokio::join!(
                check_endpoint(&app, BETA_ENDPOINT),
                check_endpoint(&app, STABLE_ENDPOINT),
            );
            let (beta, stable) = match (beta_res, stable_res) {
                (Err(b), Err(s)) => return Err(format!("beta: {b}; stable: {s}")),
                (Ok(b), Err(_)) => (b, None),
                (Err(_), Ok(s)) => (None, s),
                (Ok(b), Ok(s)) => (b, s),
            };
            pick_newer(
                beta.map(|u| (u.version.clone(), u)),
                stable.map(|u| (u.version.clone(), u)),
            )
        }
    };

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

#[cfg(test)]
mod tests {
    use super::pick_newer;

    fn pair(version: &str, label: &'static str) -> Option<(String, &'static str)> {
        Some((version.to_string(), label))
    }

    #[test]
    fn both_none_returns_none() {
        assert_eq!(pick_newer::<&str>(None, None), None);
    }

    #[test]
    fn only_a_returns_a() {
        assert_eq!(pick_newer(pair("0.1.20-beta.2", "beta"), None), Some("beta"));
    }

    #[test]
    fn only_b_returns_b() {
        assert_eq!(
            pick_newer(None, pair("0.1.20", "stable")),
            Some("stable")
        );
    }

    #[test]
    fn stable_release_beats_older_beta_prerelease() {
        // 0.1.20 > 0.1.20-beta.2 by SemVer (prereleases sort below release).
        assert_eq!(
            pick_newer(pair("0.1.20-beta.2", "beta"), pair("0.1.20", "stable")),
            Some("stable")
        );
    }

    #[test]
    fn newer_beta_beats_older_stable() {
        assert_eq!(
            pick_newer(pair("0.1.22-beta.1", "beta"), pair("0.1.21", "stable")),
            Some("beta")
        );
    }

    #[test]
    fn identical_versions_prefer_a() {
        // Call site passes beta as `a`, so a tie keeps the beta build.
        assert_eq!(
            pick_newer(pair("0.1.20", "beta"), pair("0.1.20", "stable")),
            Some("beta")
        );
    }

    #[test]
    fn unparseable_version_falls_through_to_a() {
        assert_eq!(
            pick_newer(pair("0.1.20", "beta"), pair("not-a-version", "stable")),
            Some("beta")
        );
        assert_eq!(
            pick_newer(pair("not-a-version", "beta"), pair("0.1.20", "stable")),
            Some("beta")
        );
    }
}
