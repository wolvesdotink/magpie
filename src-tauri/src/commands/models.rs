//! Whisper model-management Tauri commands and the "repair active model"
//! flow.
//!
//! Surface:
//!   - get_available_models / get_downloaded_models — registry + filesystem
//!     enumeration for the ModelPicker UI.
//!   - download_model / cancel_download — fetches GGML weights and any
//!     sibling CoreML encoder, with cancellation via the active_downloads
//!     token registry on AppState.
//!   - select_model — load an already-downloaded model and persist the
//!     selection so it auto-loads on next launch.
//!   - delete_model_file — drop the GGML file and its CoreML sibling dir;
//!     clears the selection if the deleted model was active.
//!   - repair_active_model / run_repair_active_model — re-fetches the
//!     CoreML encoder for the active model when the startup backfill
//!     failed; called from both Tauri and the tray menu.
//!
//! Correction-model commands live in commands/correction_models.rs.

use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::events::{self, event_names, TranscriptionError};
use crate::models::{downloader, registry, storage};
use crate::state::{lock_or_recover, AppState};
use crate::transcription::backend::CancellationToken;
use crate::tray;

use super::app::get_app_state_payload;

#[tauri::command]
pub fn get_available_models() -> Vec<registry::ModelInfo> {
    registry::get_available_models()
}

#[tauri::command]
pub fn get_downloaded_models() -> Result<Vec<String>, String> {
    storage::list_downloaded_models().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let model_info =
        registry::find_model(&model_id).ok_or_else(|| format!("Unknown model: {}", model_id))?;

    let encoder = model_info
        .encoder_url
        .as_deref()
        .map(|url| downloader::EncoderSpec {
            url,
            size_bytes: model_info.encoder_size_bytes.unwrap_or(0),
        });

    // Register a cancellation token so `cancel_download` can interrupt
    // the stream loop. Removed unconditionally on every exit path below.
    let cancel = CancellationToken::new();
    {
        let mut active = lock_or_recover(&state.active_downloads);
        active.insert(model_id.clone(), cancel.clone());
    }

    let result = downloader::download_model(
        &app,
        &model_id,
        &model_info.url,
        &model_info.filename,
        model_info.size_bytes,
        encoder,
        Some(&cancel),
    )
    .await;

    {
        let mut active = lock_or_recover(&state.active_downloads);
        active.remove(&model_id);
    }

    let path = match result {
        Ok(p) => p,
        Err(downloader::DownloadError::Cancelled) => {
            log::info!("Download of {} cancelled by user", model_id);
            events::emit_event(
                &app,
                event_names::MODEL_DOWNLOAD_CANCELLED,
                serde_json::json!({ "modelId": model_id }),
            );
            return Err("Download cancelled".to_string());
        }
        Err(downloader::DownloadError::Other(e)) => {
            return Err(format!("Download failed: {}", e));
        }
    };

    events::emit_event(
        &app,
        event_names::MODEL_DOWNLOAD_COMPLETE,
        serde_json::json!({ "modelId": model_id }),
    );

    // Auto-load the model after download; clean up corrupted file on failure
    if let Err(e) = load_model_internal(&app, &state, &path, &model_id) {
        log::error!("Removing corrupted model after failed load: {}", e);
        let _ = std::fs::remove_file(&path);
        return Err(format!("Model downloaded but failed to load: {}", e));
    }

    // Persist the selection so the model auto-loads on next launch
    {
        let mut settings = lock_or_recover(&state.settings);
        settings.selected_model = Some(model_id.clone());
        if let Err(e) = settings.save() {
            log::error!("Failed to save settings: {}", e);
        }
    }

    // Refresh tray menu so the new model appears as downloaded
    tray::rebuild_tray_menu(&app);

    Ok(())
}

/// Signals the in-flight download for `model_id` to abort. Idempotent —
/// returns Ok even if no download is currently registered (covers the race
/// between cancel and completion).
#[tauri::command]
pub fn cancel_download(state: State<'_, Arc<AppState>>, model_id: String) -> Result<(), String> {
    let active = lock_or_recover(&state.active_downloads);
    if let Some(token) = active.get(&model_id) {
        token.cancel();
        log::info!("Cancel requested for download {}", model_id);
    } else {
        log::debug!("Cancel requested for {} but no active download", model_id);
    }
    Ok(())
}

#[tauri::command]
pub fn select_model(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let model_info =
        registry::find_model(&model_id).ok_or_else(|| format!("Unknown model: {}", model_id))?;

    let path = storage::model_path(&model_info.filename).map_err(|e| e.to_string())?;

    if !path.exists() {
        return Err(format!("Model {} is not downloaded", model_id));
    }

    load_model_internal(&app, &state, &path, &model_id)?;

    // Save preference
    {
        let mut settings = lock_or_recover(&state.settings);
        settings.selected_model = Some(model_id);
        if let Err(e) = settings.save() {
            log::error!("Failed to save settings: {}", e);
        }
    }

    // Refresh tray menu so the checkmark updates
    tray::rebuild_tray_menu(&app);

    Ok(())
}

#[tauri::command]
pub fn delete_model_file(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let model_info =
        registry::find_model(&model_id).ok_or_else(|| format!("Unknown model: {}", model_id))?;

    storage::delete_model(&model_info.filename).map_err(|e| e.to_string())?;

    // Best-effort cleanup of the sibling CoreML encoder directory if one
    // was downloaded with this model.
    let encoder_dir_name = downloader::encoder_dir_name_from_filename(&model_info.filename);
    if let Ok(models_dir) = storage::models_dir() {
        let encoder_path = models_dir.join(&encoder_dir_name);
        if encoder_path.exists() {
            if let Err(e) = std::fs::remove_dir_all(&encoder_path) {
                log::warn!(
                    "Failed to remove CoreML encoder dir {}: {}",
                    encoder_path.display(),
                    e
                );
            }
        }
    }

    // If the deleted model was the active one, clear the selection
    {
        let mut settings = lock_or_recover(&state.settings);
        if settings.selected_model.as_deref() == Some(&model_id) {
            settings.selected_model = None;
            if let Err(e) = settings.save() {
                log::error!("Failed to save settings: {}", e);
            }
        }
    }

    // Refresh tray menu so the deleted model appears grayed out
    tray::rebuild_tray_menu(&app);

    Ok(())
}

/// Shared "load model into AppState, emit state-changed event" path. Used by
/// both `download_model` (post-fetch) and `select_model` (when the user
/// switches to an already-downloaded model).
fn load_model_internal(
    app: &AppHandle,
    state: &State<'_, Arc<AppState>>,
    path: &std::path::Path,
    model_id: &str,
) -> Result<(), String> {
    let (backend, self_test) =
        crate::load_with_self_test(path).map_err(|e| format!("Failed to load model: {}", e))?;

    if let Err(e) = self_test {
        log::warn!(
            "Self-test for model {} returned: {}. Continuing — \
             first real transcription will surface any actual issue.",
            model_id,
            e
        );
    }

    // backend(#2) and current_model_path(#2) share rank 2 — sequential
    // single-statement acquires drop the first guard at the `;` before
    // the second runs, so this never co-holds and is protocol-safe.
    *lock_or_recover(&state.backend) = Some(backend);
    *lock_or_recover(&state.current_model_path) = Some(path.to_path_buf());

    log::info!("Model {} loaded successfully", model_id);
    tray::set_tray_status(app, "Magpie — Ready");

    // Emit state change
    events::emit_event(
        app,
        event_names::APP_STATE_CHANGED,
        get_app_state_payload(state),
    );

    Ok(())
}

// ── Repair Active Model ────────────────────────────────────────────

/// Re-fetch the CoreML encoder package for the currently loaded model and
/// reload the `WhisperBackend` so the encoder is picked up. Used by the
/// tray "Repair Active Model" item as the manual recovery path when the
/// startup backfill fails (network blip, server 5xx, partial unzip).
///
/// Refuses to run while recording or processing is in flight — swapping
/// the backend mid-inference would crash the active call.
#[tauri::command]
pub async fn repair_active_model(
    app: AppHandle,
    _state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    run_repair_active_model(&app).await;
    Ok(())
}

/// AppHandle-only entry point so non-Tauri callers (the tray menu handler)
/// don't need to thread a `State<'_, Arc<AppState>>` reference.
pub async fn run_repair_active_model(app: &AppHandle) {
    let state = app.state::<Arc<AppState>>();

    if state.is_recording() || state.is_processing() {
        log::warn!("Repair Active Model: refusing while recording or processing");
        events::emit_event(
            app,
            event_names::TRANSCRIPTION_ERROR,
            TranscriptionError {
                error: "Cannot repair model while recording or processing".to_string(),
            },
        );
        return;
    }

    let model_id = match lock_or_recover(&state.settings).selected_model.clone() {
        Some(id) => id,
        None => {
            log::warn!("Repair Active Model: no model selected");
            events::emit_event(
                app,
                event_names::TRANSCRIPTION_ERROR,
                TranscriptionError {
                    error: "No active model to repair".to_string(),
                },
            );
            return;
        }
    };
    let info = match registry::find_model(&model_id) {
        Some(i) => i,
        None => {
            log::warn!("Repair Active Model: unknown model id '{}'", model_id);
            return;
        }
    };
    let encoder_url = match info.encoder_url.clone() {
        Some(u) => u,
        None => {
            log::info!(
                "Repair Active Model: '{}' has no CoreML encoder (Distil-Whisper or similar) — nothing to repair",
                model_id
            );
            return;
        }
    };
    let encoder_size = info.encoder_size_bytes.unwrap_or(0);
    let models_dir = match storage::models_dir() {
        Ok(d) => d,
        Err(e) => {
            log::error!("Repair Active Model: cannot resolve models dir: {}", e);
            return;
        }
    };
    let encoder_dir = models_dir.join(downloader::encoder_dir_name_from_filename(&info.filename));
    let model_path = match storage::model_path(&info.filename) {
        Ok(p) if p.exists() => p,
        _ => {
            log::error!(
                "Repair Active Model: model file missing for '{}' — re-download it instead",
                model_id
            );
            return;
        }
    };

    // Drop a stale `.mlmodelc.broken` from a prior (now-removed) quarantine
    // path so the post-download reload can't pick the broken artifact back
    // up. New builds don't create these, but old user installs may still
    // have one on disk.
    {
        let stem = model_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let broken_dir = model_path.with_file_name(format!("{}-encoder.mlmodelc.broken", stem));
        if broken_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&broken_dir) {
                log::warn!(
                    "Repair Active Model: could not remove {}: {}",
                    broken_dir.display(),
                    e
                );
            } else {
                log::info!(
                    "Repair Active Model: removed stale {}",
                    broken_dir.display()
                );
            }
        }
    }

    log::info!(
        "Repairing model '{}' — re-downloading CoreML encoder",
        model_id
    );
    match downloader::download_encoder_only(
        app,
        &model_id,
        &encoder_url,
        encoder_size,
        &encoder_dir,
        None,
    )
    .await
    {
        Ok(()) => {
            log::info!(
                "Repair Active Model: encoder restored for '{}'; reloading backend",
                model_id
            );
            // Reuse the same idle-aware reload helper used by the startup
            // backfill path. Because we early-returned on busy state above,
            // the reload runs synchronously here. The reload itself runs the
            // self-test and re-quarantines the encoder if it still fails.
            crate::reload_backend_after_backfill_public(
                app.clone(),
                state.inner().clone(),
                model_id,
                model_path,
            )
            .await;
        }
        Err(e) => {
            log::error!("Repair Active Model: encoder download failed: {}", e);
            events::emit_event(
                app,
                event_names::TRANSCRIPTION_ERROR,
                TranscriptionError {
                    error: format!("Repair failed: {}", e),
                },
            );
        }
    }
}
