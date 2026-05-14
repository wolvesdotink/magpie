//! Whisper model-management Tauri commands.
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
//!
//! CoreML encoder recovery is fully automatic — see
//! `maybe_backfill_coreml_encoder` in `model_loading.rs` for the retry loop.
//!
//! Correction-model commands live in commands/correction_models.rs.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::command_error::CommandError;
use crate::events::{self, event_names};
use crate::models::{downloader, registry, storage, ModelError};
use crate::state::{lock_or_recover, AppState};
use crate::transcription::backend::CancellationToken;
use crate::tray;

use super::app::get_app_state_payload;

#[tauri::command]
pub fn get_available_models() -> Vec<registry::ModelInfo> {
    registry::get_available_models()
}

#[tauri::command]
pub fn get_downloaded_models() -> Result<Vec<String>, CommandError> {
    Ok(storage::list_downloaded_models()?)
}

#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), CommandError> {
    let model_info =
        registry::find_model(&model_id).ok_or(ModelError::UnknownId(model_id.clone()))?;

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
        Err(ModelError::Cancelled) => {
            log::info!("Download of {} cancelled by user", model_id);
            events::emit_event(
                &app,
                event_names::MODEL_DOWNLOAD_CANCELLED,
                serde_json::json!({ "modelId": model_id }),
            );
            return Err(CommandError::Cancelled);
        }
        Err(e) => {
            return Err(e.into());
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
        return Err(e);
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
pub fn cancel_download(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), CommandError> {
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
) -> Result<(), CommandError> {
    let model_info =
        registry::find_model(&model_id).ok_or(ModelError::UnknownId(model_id.clone()))?;

    let path = storage::model_path(&model_info.filename)?;

    if !path.exists() {
        return Err(ModelError::UnknownId(format!("{model_id} (file missing on disk)")).into());
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
) -> Result<(), CommandError> {
    let model_info =
        registry::find_model(&model_id).ok_or(ModelError::UnknownId(model_id.clone()))?;

    storage::delete_model(&model_info.filename)?;

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
) -> Result<(), CommandError> {
    let (backend, self_test) =
        crate::load_with_self_test(path).map_err(|e| CommandError::Transcription {
            message: format!("Failed to load model: {e}"),
        })?;

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
