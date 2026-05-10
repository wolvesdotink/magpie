use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::audio;
use crate::correction;
use crate::correction_detector;
use crate::events::{self, event_names, AppStatePayload, AudioAmplitudePayload, PermissionsPayload, TranscriptionError, TranscriptionResult};
use crate::hotkey;
use crate::models::{downloader, registry, storage};
use crate::output;
use crate::state::AppState;
use crate::transcription::{engine, postprocess};
use crate::overlay;
use crate::tray::{self, TrayState};
use crate::vocabulary::{VocabularyEntry, VocabularySource};

// ── Recording Controls ──────────────────────────────────────────────

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if state.is_recording() {
        return Err("Already recording".to_string());
    }

    if state.is_processing() {
        return Err("Currently processing".to_string());
    }

    let state_arc = state.inner().clone();
    let (stream, sample_rate) = audio::capture::start_recording(&state_arc)
        .map_err(|e| format!("Failed to start recording: {}", e))?;

    {
        let mut sr = state.capture_sample_rate.lock().unwrap();
        *sr = sample_rate;
    }
    {
        let mut active = state.active_stream.lock().unwrap();
        *active = Some(stream);
    }

    state.set_recording(true);
    tray::set_tray_icon(&app, TrayState::Recording);
    tray::set_tray_status(&app, "Magpie — Recording...");
    overlay::show_overlay(&app);
    events::emit_event(&app, event_names::RECORDING_STARTED, ());

    // Spawn amplitude emitter thread (20Hz = every 50ms)
    {
        let emitter_app = app.clone();
        let emitter_state = state_arc.clone();
        std::thread::spawn(move || {
            while emitter_state.is_recording() {
                let rms = emitter_state.get_amplitude();

                // Scale and clamp: raw RMS from speech is typically 0.001–0.1.
                // Multiply by 8 and clamp to [0, 1] for a usable range.
                // Apply a mild power curve for better perceptual mapping.
                let scaled = (rms * 8.0).min(1.0);
                let normalized = scaled.powf(0.7);

                events::emit_event(
                    &emitter_app,
                    event_names::AUDIO_AMPLITUDE,
                    AudioAmplitudePayload { amplitude: normalized },
                );

                std::thread::sleep(std::time::Duration::from_millis(50));
            }

            // Emit a final zero so the frontend can animate bars down to rest
            events::emit_event(
                &emitter_app,
                event_names::AUDIO_AMPLITUDE,
                AudioAmplitudePayload { amplitude: 0.0 },
            );

            log::debug!("Amplitude emitter thread exiting");
        });
    }

    log::info!("Recording started at {} Hz", sample_rate);
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if !state.is_recording() {
        return Err("Not recording".to_string());
    }

    // Drop the stream to stop recording
    {
        let mut active = state.active_stream.lock().unwrap();
        *active = None;
    }

    state.set_recording(false);
    state.set_processing(true);
    tray::set_tray_icon(&app, TrayState::Processing);
    tray::set_tray_status(&app, "Magpie — Transcribing...");
    events::emit_event(&app, event_names::RECORDING_STOPPED, ());
    events::emit_event(&app, event_names::TRANSCRIPTION_STARTED, ());

    // Get the audio buffer and sample rate
    let audio_data = {
        let mut buffer = state.audio_buffer.lock().unwrap();
        let data = buffer.clone();
        buffer.clear();
        data
    };

    let sample_rate = *state.capture_sample_rate.lock().unwrap();

    log::info!(
        "Recorded {} samples ({:.1}s) at {} Hz",
        audio_data.len(),
        audio_data.len() as f64 / sample_rate as f64,
        sample_rate
    );

    // Spawn transcription on a background thread
    let state_arc = state.inner().clone();
    let app_clone = app.clone();

    tokio::task::spawn_blocking(move || {
        // Resample to 16kHz
        let resampled = audio::resample::resample_to_16khz(&audio_data, sample_rate);

        // Get language setting and vocabulary data
        let language = {
            let settings = state_arc.settings.lock().unwrap();
            settings.language.clone()
        };

        let (initial_prompt, vocab_replacements) = {
            let vocab = state_arc.vocabulary.lock().unwrap();
            (vocab.get_initial_prompt_words(), vocab.get_replacements())
        };

        let prompt_ref = if initial_prompt.is_empty() {
            None
        } else {
            Some(initial_prompt.as_str())
        };

        // Transcribe
        let ctx_guard = state_arc.whisper_context.lock().unwrap();
        if let Some(ctx) = ctx_guard.as_ref() {
            match engine::transcribe(ctx, &resampled, language.as_deref(), prompt_ref) {
                Ok((raw_text, duration_ms)) => {
                    // Post-process
                    let (filler_words, remove_fillers) = {
                        let settings = state_arc.settings.lock().unwrap();
                        (settings.filler_words.clone(), settings.remove_fillers)
                    };

                    let text = postprocess::postprocess(&raw_text, &filler_words, remove_fillers, &vocab_replacements);

                    // Self-correction cleanup (if enabled)
                    let text = {
                        let self_correction_enabled = {
                            let settings = state_arc.settings.lock().unwrap();
                            settings.self_correction
                        };

                        if self_correction_enabled && !text.is_empty() {
                            tray::set_tray_status(&app_clone, "Magpie \u{2014} Cleaning up...");
                            events::emit_event(
                                &app_clone,
                                event_names::CORRECTION_STARTED,
                                (),
                            );

                            let backend_guard = state_arc.llama_backend.lock().unwrap();
                            let model_guard = state_arc.correction_model.lock().unwrap();

                            if let (Some(ref backend), Some(ref model)) =
                                (&*backend_guard, &*model_guard)
                            {
                                match correction::engine::correct_transcription(
                                    backend, model, &text,
                                ) {
                                    Ok(corrected) if !corrected.is_empty() => {
                                        log::info!(
                                            "Self-correction: \"{}\" -> \"{}\"",
                                            text,
                                            corrected
                                        );
                                        events::emit_event(
                                            &app_clone,
                                            event_names::CORRECTION_COMPLETE,
                                            (),
                                        );
                                        corrected
                                    }
                                    Ok(_) => {
                                        log::warn!(
                                            "Correction produced empty text, using original"
                                        );
                                        text
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "Correction failed, using original: {}",
                                            e
                                        );
                                        text
                                    }
                                }
                            } else {
                                log::debug!(
                                    "Self-correction enabled but no model loaded"
                                );
                                text
                            }
                        } else {
                            text
                        }
                    };

                    if !text.is_empty() {
                        // Store last transcription
                        {
                            let mut last = state_arc.last_transcription.lock().unwrap();
                            *last = text.clone();
                        }

                        // Paste into active app
                        if let Err(e) = output::paste::paste_text(&app_clone, &text) {
                            log::error!("Failed to paste text: {}", e);
                        } else {
                            log::info!("Pasted {} chars", text.len());

                            // Start correction detection if vocabulary learning is enabled
                            let vocab_learning_enabled = {
                                let settings = state_arc.settings.lock().unwrap();
                                settings.vocabulary_learning
                            };
                            if vocab_learning_enabled {
                                correction_detector::start_detection(
                                    text.clone(),
                                    state_arc.clone(),
                                    app_clone.clone(),
                                );
                            }
                        }

                        events::emit_event(
                            &app_clone,
                            event_names::TRANSCRIPTION_COMPLETE,
                            TranscriptionResult {
                                text,
                                duration_ms,
                            },
                        );
                    } else {
                        log::info!("Transcription produced empty text after post-processing");
                        events::emit_event(
                            &app_clone,
                            event_names::TRANSCRIPTION_COMPLETE,
                            TranscriptionResult {
                                text: String::new(),
                                duration_ms,
                            },
                        );
                    }
                }
                Err(e) => {
                    log::error!("Transcription failed: {}", e);
                    events::emit_event(
                        &app_clone,
                        event_names::TRANSCRIPTION_ERROR,
                        TranscriptionError {
                            error: e.to_string(),
                        },
                    );
                }
            }
        } else {
            log::warn!("Cannot transcribe: no whisper model loaded");
            events::emit_event(
                &app_clone,
                event_names::TRANSCRIPTION_ERROR,
                TranscriptionError {
                    error: "No model loaded".to_string(),
                },
            );
        }

        state_arc.set_processing(false);
        tray::set_tray_icon(&app_clone, TrayState::Idle);
        tray::set_tray_status(&app_clone, "Magpie — Ready");
        overlay::hide_overlay(&app_clone);
    });

    Ok(())
}

#[tauri::command]
pub async fn toggle_recording(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if state.is_recording() {
        stop_recording(app, state).await
    } else {
        start_recording(app, state).await
    }
}

// ── App State ──────────────────────────────────────────────────────

#[tauri::command]
pub fn get_app_state(state: State<'_, Arc<AppState>>) -> AppStatePayload {
    let has_model = state.whisper_context.lock().unwrap().is_some();
    let last_transcription = state.last_transcription.lock().unwrap().clone();

    AppStatePayload {
        recording: state.is_recording(),
        processing: state.is_processing(),
        has_model,
        last_transcription,
    }
}

// ── Model Management ───────────────────────────────────────────────

#[tauri::command]
pub fn get_available_models() -> Vec<registry::ModelInfo> {
    let models = registry::get_available_models();

    // Mark which ones are downloaded
    models
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
    let model_info = registry::find_model(&model_id)
        .ok_or_else(|| format!("Unknown model: {}", model_id))?;

    let path = downloader::download_model(
        &app,
        &model_id,
        &model_info.url,
        &model_info.filename,
        model_info.size_bytes,
    )
    .await
    .map_err(|e| format!("Download failed: {}", e))?;

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
        let mut settings = state.settings.lock().unwrap();
        settings.selected_model = Some(model_id.clone());
        if let Err(e) = settings.save() {
            log::error!("Failed to save settings: {}", e);
        }
    }

    // Refresh tray menu so the new model appears as downloaded
    tray::rebuild_tray_menu(&app);

    Ok(())
}

#[tauri::command]
pub fn select_model(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let model_info = registry::find_model(&model_id)
        .ok_or_else(|| format!("Unknown model: {}", model_id))?;

    let path = storage::model_path(&model_info.filename)
        .map_err(|e| e.to_string())?;

    if !path.exists() {
        return Err(format!("Model {} is not downloaded", model_id));
    }

    load_model_internal(&app, &state, &path, &model_id)?;

    // Save preference
    {
        let mut settings = state.settings.lock().unwrap();
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
    let model_info = registry::find_model(&model_id)
        .ok_or_else(|| format!("Unknown model: {}", model_id))?;

    storage::delete_model(&model_info.filename).map_err(|e| e.to_string())?;

    // If the deleted model was the active one, clear the selection
    {
        let mut settings = state.settings.lock().unwrap();
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

fn load_model_internal(
    app: &AppHandle,
    state: &State<'_, Arc<AppState>>,
    path: &std::path::Path,
    model_id: &str,
) -> Result<(), String> {
    let ctx = engine::load_model(path).map_err(|e| format!("Failed to load model: {}", e))?;

    {
        let mut whisper_ctx = state.whisper_context.lock().unwrap();
        *whisper_ctx = Some(ctx);
    }
    {
        let mut model_path = state.current_model_path.lock().unwrap();
        *model_path = Some(path.to_path_buf());
    }

    log::info!("Model {} loaded successfully", model_id);
    tray::set_tray_status(app, "Magpie — Ready");

    // Emit state change
    events::emit_event(app, event_names::APP_STATE_CHANGED, get_app_state_payload(state));

    Ok(())
}

fn get_app_state_payload(state: &State<'_, Arc<AppState>>) -> AppStatePayload {
    let has_model = state.whisper_context.lock().unwrap().is_some();
    let last_transcription = state.last_transcription.lock().unwrap().clone();

    AppStatePayload {
        recording: state.is_recording(),
        processing: state.is_processing(),
        has_model,
        last_transcription,
    }
}

// ── Permissions ────────────────────────────────────────────────────

#[tauri::command]
pub fn check_permissions() -> PermissionsPayload {
    let mic_status = crate::permissions::microphone_authorization_status();
    PermissionsPayload {
        microphone: mic_status == crate::permissions::MicrophoneAuthStatus::Authorized,
        accessibility: crate::permissions::is_accessibility_trusted(),
        input_monitoring: crate::permissions::is_input_monitoring_trusted(),
    }
}

#[tauri::command]
pub fn request_microphone_permission() -> bool {
    crate::permissions::request_microphone_access()
}

#[tauri::command]
pub fn open_microphone_settings(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.suppress_hide.store(true, std::sync::atomic::Ordering::SeqCst);
    crate::permissions::open_microphone_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_accessibility_settings(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.suppress_hide.store(true, std::sync::atomic::Ordering::SeqCst);
    crate::permissions::open_accessibility_settings().map_err(|e| e.to_string())
}

/// Trigger the Input Monitoring TCC prompt. On first call this prompts the
/// user and adds the app to System Settings → Privacy & Security → Input
/// Monitoring so the toggle becomes available. After the user grants it,
/// the Fn key monitor must be restarted to pick up the new permission.
#[tauri::command]
pub fn request_input_monitoring_permission() -> bool {
    crate::permissions::request_input_monitoring_access()
}

#[tauri::command]
pub fn open_input_monitoring_settings(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.suppress_hide.store(true, std::sync::atomic::Ordering::SeqCst);
    crate::permissions::open_input_monitoring_settings().map_err(|e| e.to_string())
}

/// Restart the app. Re-execs the current binary; on macOS this lets a freshly
/// granted Accessibility permission take effect when AX trust was cached as
/// `false` for the previous process lifetime.
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    app.restart();
}

// ── Hotkey ─────────────────────────────────────────────────────────

/// Rust-callable helper that stops any running Fn key monitor and starts a
/// fresh one. Factored out so the watchdog in `lib.rs` can call it without
/// going through the Tauri command machinery (which requires `State<'_, _>`).
pub fn restart_fn_key_monitor_inner(
    app: &AppHandle,
    state: &Arc<AppState>,
) -> Result<bool, String> {
    // Stop the existing monitor before starting a new one
    if let Ok(mut guard) = state.fn_key_monitor.lock() {
        if let Some(handle) = guard.take() {
            handle.stop();
        }
    }

    let activation_mode = state
        .settings
        .lock()
        .map(|s| s.activation_mode.clone())
        .unwrap_or_default();

    // Clone the recording command sender for the new monitor
    let tx = match state.recording_tx.lock() {
        Ok(guard) => match guard.as_ref() {
            Some(tx) => tx.clone(),
            None => {
                return Err(
                    "Cannot restart fn key monitor: recording channel not initialized".into(),
                );
            }
        },
        Err(e) => {
            return Err(format!("recording_tx mutex poisoned: {}", e));
        }
    };

    let (new_handle, tap_ok) = hotkey::start_fn_key_monitor(app.clone(), tx, activation_mode);
    if let Ok(mut guard) = state.fn_key_monitor.lock() {
        *guard = Some(new_handle);
    }

    if !tap_ok {
        log::warn!("restart_fn_key_monitor: CGEventTap creation failed");
    }

    Ok(tap_ok)
}

#[tauri::command]
pub fn restart_fn_key_monitor(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    restart_fn_key_monitor_inner(&app, state.inner())
}

#[tauri::command]
pub fn get_fn_key_monitor_status(state: State<'_, Arc<AppState>>) -> bool {
    state
        .fn_key_monitor
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|h| h.is_active()))
        .unwrap_or(false)
}

// ── Settings ───────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> crate::settings::UserSettings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, Arc<AppState>>,
    settings: crate::settings::UserSettings,
) {
    let mut current = state.settings.lock().unwrap();
    *current = settings;
    if let Err(e) = current.save() {
        log::error!("Failed to save settings: {}", e);
    }
}

// ── Correction Model Management ───────────────────────────────────

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

    let path = downloader::download_model(
        &app,
        &model_id,
        &model_info.url,
        &model_info.filename,
        model_info.size_bytes,
    )
    .await
    .map_err(|e| format!("Download failed: {}", e))?;

    events::emit_event(
        &app,
        event_names::MODEL_DOWNLOAD_COMPLETE,
        serde_json::json!({ "modelId": model_id }),
    );

    // Auto-load the model after download.
    // On failure, keep the file (it is likely valid) but report the error so the
    // user can retry without re-downloading.
    if let Err(e) = load_correction_model_internal(&app, &state, &path, &model_id) {
        log::error!("Correction model downloaded but failed to load: {}", e);
        return Err(format!("Model downloaded but failed to load: {}", e));
    }

    // Persist the selection so the correction model auto-loads on next launch
    {
        let mut settings = state.settings.lock().unwrap();
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
        let mut settings = state.settings.lock().unwrap();
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
        let mut settings = state.settings.lock().unwrap();
        if settings.selected_correction_model.as_deref() == Some(&model_id) {
            settings.selected_correction_model = None;
            if let Err(e) = settings.save() {
                log::error!("Failed to save settings: {}", e);
            }
        }
    }
    {
        let current_path = state.current_correction_model_path.lock().unwrap();
        if current_path.is_some() {
            drop(current_path);
            let mut cm = state.correction_model.lock().unwrap();
            *cm = None;
            let mut cp = state.current_correction_model_path.lock().unwrap();
            *cp = None;
        }
    }

    // Emit state change
    events::emit_event(&app, event_names::APP_STATE_CHANGED, get_app_state_payload(&state));

    Ok(())
}

fn load_correction_model_internal(
    app: &AppHandle,
    state: &State<'_, Arc<AppState>>,
    path: &std::path::Path,
    model_id: &str,
) -> Result<(), String> {
    // Initialize llama backend if needed
    {
        let mut backend_guard = state.llama_backend.lock().unwrap();
        if backend_guard.is_none() {
            let backend = llama_cpp_2::llama_backend::LlamaBackend::init()
                .map_err(|e| format!("Failed to init llama backend: {:?}", e))?;
            *backend_guard = Some(backend);
        }
    }

    let backend_guard = state.llama_backend.lock().unwrap();
    let backend = backend_guard
        .as_ref()
        .ok_or_else(|| "Llama backend not initialized".to_string())?;

    let model = correction::engine::load_correction_model(backend, path)
        .map_err(|e| format!("Failed to load correction model: {}", e))?;

    {
        let mut cm = state.correction_model.lock().unwrap();
        *cm = Some(model);
    }
    {
        let mut cp = state.current_correction_model_path.lock().unwrap();
        *cp = Some(path.to_path_buf());
    }

    log::info!("Correction model {} loaded successfully", model_id);

    // Emit state change
    events::emit_event(
        app,
        event_names::APP_STATE_CHANGED,
        get_app_state_payload(&state),
    );

    Ok(())
}

// ── Vocabulary ────────────────────────────────────────────────────

#[tauri::command]
pub fn get_vocabulary(state: State<'_, Arc<AppState>>) -> Vec<VocabularyEntry> {
    state.vocabulary.lock().unwrap().entries.clone()
}

#[tauri::command]
pub fn add_vocabulary_entry(
    state: State<'_, Arc<AppState>>,
    wrong: String,
    correct: String,
) -> Result<(), String> {
    let mut vocab = state.vocabulary.lock().unwrap();
    vocab.add_or_update(&wrong, &correct, VocabularySource::Manual);
    vocab.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_vocabulary_entry(
    state: State<'_, Arc<AppState>>,
    wrong: String,
) -> Result<(), String> {
    let mut vocab = state.vocabulary.lock().unwrap();
    vocab.remove(&wrong);
    vocab.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_vocabulary(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut vocab = state.vocabulary.lock().unwrap();
    vocab.entries.clear();
    vocab.save().map_err(|e| e.to_string())
}
