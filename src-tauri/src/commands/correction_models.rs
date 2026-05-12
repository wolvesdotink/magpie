//! Correction-model Tauri commands.
//!
//! Correction models are small LLMs (Qwen2.5-0.5B today) used by the
//! `correction_detector` flow to detect and apply user corrections after
//! a paste. They share the on-disk storage path with whisper models but
//! load through llama.cpp rather than whisper.cpp.
//!
//! Surface mirrors commands/models.rs for whisper:
//!   - get_available_correction_models / get_downloaded_correction_models
//!   - download_correction_model — same download flow as whisper, but no
//!     CoreML encoder sibling.
//!   - select_correction_model — load an already-downloaded model.
//!   - delete_correction_model_file — drop the file; clear the selection
//!     and unload from AppState if the deleted model was active.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::correction;
use crate::events::{self, event_names};
use crate::models::{downloader, storage};
use crate::state::{lock_or_recover, AppState};
use crate::transcription::backend::CancellationToken;

use super::app::get_app_state_payload;

#[tauri::command]
pub fn get_available_correction_models() -> Vec<correction::registry::CorrectionModelInfo> {
    correction::registry::get_available_correction_models()
}

#[tauri::command]
pub fn get_downloaded_correction_models() -> Result<Vec<String>, String> {
    storage::list_downloaded_correction_models().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_correction_model(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let model_info = correction::registry::find_correction_model(&model_id)
        .ok_or_else(|| format!("Unknown correction model: {}", model_id))?;

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
        None,
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
            log::info!(
                "Correction-model download of {} cancelled by user",
                model_id
            );
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

    // Auto-load the model after download. On failure, keep the file (it
    // is likely valid) but report the error so the user can retry without
    // re-downloading.
    if let Err(e) = load_correction_model_internal(&app, &state, &path, &model_id) {
        log::error!("Correction model downloaded but failed to load: {}", e);
        return Err(format!("Model downloaded but failed to load: {}", e));
    }

    // Persist the selection so the correction model auto-loads on next launch
    {
        let mut settings = lock_or_recover(&state.settings);
        settings.selected_correction_model = Some(model_id.clone());
        if let Err(e) = settings.save() {
            log::error!("Failed to save settings: {}", e);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn select_correction_model(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let model_info = correction::registry::find_correction_model(&model_id)
        .ok_or_else(|| format!("Unknown correction model: {}", model_id))?;

    let path = storage::model_path(&model_info.filename).map_err(|e| e.to_string())?;

    if !path.exists() {
        return Err(format!("Correction model {} is not downloaded", model_id));
    }

    load_correction_model_internal(&app, &state, &path, &model_id)?;

    // Save preference
    {
        let mut settings = lock_or_recover(&state.settings);
        settings.selected_correction_model = Some(model_id);
        if let Err(e) = settings.save() {
            log::error!("Failed to save settings: {}", e);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn delete_correction_model_file(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let model_info = correction::registry::find_correction_model(&model_id)
        .ok_or_else(|| format!("Unknown correction model: {}", model_id))?;

    storage::delete_correction_model(&model_info.filename).map_err(|e| e.to_string())?;

    // If the deleted model was the active one, clear the selection and unload
    {
        let mut settings = lock_or_recover(&state.settings);
        if settings.selected_correction_model.as_deref() == Some(&model_id) {
            settings.selected_correction_model = None;
            if let Err(e) = settings.save() {
                log::error!("Failed to save settings: {}", e);
            }
        }
    }
    {
        let current_path = lock_or_recover(&state.current_correction_model_path);
        if current_path.is_some() {
            drop(current_path);
            let mut cm = lock_or_recover(&state.correction_model);
            *cm = None;
            let mut cp = lock_or_recover(&state.current_correction_model_path);
            *cp = None;
        }
    }

    // Emit state change
    events::emit_event(
        &app,
        event_names::APP_STATE_CHANGED,
        get_app_state_payload(&state),
    );

    Ok(())
}

/// Shared "load correction model into AppState, emit state-changed event"
/// path. Used by both `download_correction_model` (post-fetch) and
/// `select_correction_model` (when the user switches to an already-
/// downloaded model). Initializes `llama_backend` on first call.
fn load_correction_model_internal(
    app: &AppHandle,
    state: &State<'_, Arc<AppState>>,
    path: &std::path::Path,
    model_id: &str,
) -> Result<(), String> {
    // Initialize llama backend if needed
    {
        let mut backend_guard = lock_or_recover(&state.llama_backend);
        if backend_guard.is_none() {
            let backend = llama_cpp_2::llama_backend::LlamaBackend::init()
                .map_err(|e| format!("Failed to init llama backend: {:?}", e))?;
            *backend_guard = Some(backend);
        }
    }

    let backend_guard = lock_or_recover(&state.llama_backend);
    let backend = backend_guard
        .as_ref()
        .ok_or_else(|| "Llama backend not initialized".to_string())?;

    let model = correction::engine::load_correction_model(backend, path)
        .map_err(|e| format!("Failed to load correction model: {}", e))?;

    {
        let mut cm = lock_or_recover(&state.correction_model);
        *cm = Some(model);
    }
    {
        let mut cp = lock_or_recover(&state.current_correction_model_path);
        *cp = Some(path.to_path_buf());
    }

    log::info!("Correction model {} loaded successfully", model_id);
    drop(backend_guard); // release rank-9 before re-entering rank-2 via get_app_state_payload

    // Emit state change
    events::emit_event(
        app,
        event_names::APP_STATE_CHANGED,
        get_app_state_payload(state),
    );

    Ok(())
}
