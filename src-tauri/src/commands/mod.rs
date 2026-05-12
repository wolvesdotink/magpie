// Domain-extracted command modules. Each holds the Tauri command handlers
// for one slice of the app's surface (vocabulary, recording, models, …).
// `pub use` re-exports them so `commands::name` keeps working from
// lib.rs's `invoke_handler!` macro and from cross-module callers like
// tray.rs's `commands::run_repair_active_model`.
pub mod vocabulary;
pub use vocabulary::*;

use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::audio;
use crate::correction;
use crate::correction_detector;
use crate::events::{
    self, event_names, AppStatePayload, AudioAmplitudePayload, PermissionsPayload,
    TranscriptionError, TranscriptionResult,
};
use crate::hotkey;
use crate::models::{downloader, registry, storage};
use crate::output;
use crate::state::{lock_or_recover, AppState};
use crate::transcription::backend::{CancellationToken, TranscribeMode, TranscribeOptions};
use crate::transcription::postprocess;
use crate::transcription::streaming;

use crate::overlay;
use crate::tray::{self, TrayState};

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
        let mut sr = lock_or_recover(&state.capture_sample_rate);
        *sr = sample_rate;
    }
    {
        let mut active = lock_or_recover(&state.active_stream);
        *active = Some(stream);
    }

    state.set_recording(true);
    tray::set_tray_icon(&app, TrayState::Recording);
    tray::set_tray_status(&app, "Magpie — Recording...");
    overlay::show_overlay(&app);
    events::emit_event(&app, event_names::RECORDING_STARTED, ());

    // Spawn the streaming-preview worker. It will poll the audio buffer
    // every ~1.5s and emit PARTIAL_TRANSCRIPTION events the overlay can
    // render as a live caption. Skip if no backend is loaded — there's
    // nothing to decode against — or if the user has disabled the live
    // preview in Settings (default off; final-on-stop is unaffected).
    let backend_present = state.backend.lock().map(|g| g.is_some()).unwrap_or(false);
    let streaming_enabled = state
        .settings
        .lock()
        .map(|s| s.streaming_preview)
        .unwrap_or(false);
    if backend_present && streaming_enabled {
        let handle = streaming::spawn_streaming_worker(app.clone(), state_arc.clone());
        if let Ok(mut slot) = state.streaming_handle.lock() {
            *slot = Some(handle);
        }
    } else if !backend_present {
        log::debug!("Streaming worker not started: no backend loaded");
    } else {
        log::debug!("Streaming worker not started: live preview disabled in settings");
    }

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
                    AudioAmplitudePayload {
                        amplitude: normalized,
                    },
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

    // Bind Escape as a global cancel shortcut for the lifetime of this
    // recording. Torn down in stop_recording / cancel_recording. Best-effort:
    // a failure here (e.g. user's custom shortcut is also Escape) is logged
    // but does not block the recording from continuing.
    if let Err(e) = register_escape_shortcut(&app) {
        log::warn!("Could not register Escape cancel shortcut: {}", e);
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_recording(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if !state.is_recording() {
        return Err("Not recording".to_string());
    }

    // Drop the stream to stop recording
    {
        let mut active = lock_or_recover(&state.active_stream);
        *active = None;
    }

    state.set_recording(false);

    // Tear down the streaming worker BEFORE flipping `processing` so a
    // stale partial can't race the final pass. partial_cancel aborts the
    // in-flight whisper.cpp call (via the abort_callback wired in the
    // backend); cancel signals the loop to exit. The 2s timeout bounds
    // the wait if a stuck inference somehow ignores the abort.
    let streaming_handle = state
        .streaming_handle
        .lock()
        .ok()
        .and_then(|mut g| g.take());
    if let Some(h) = streaming_handle {
        h.partial_cancel.cancel();
        h.cancel.cancel();
        let _ = tokio::time::timeout(std::time::Duration::from_millis(2000), h.join).await;
    }

    state.set_processing(true);
    tray::set_tray_icon(&app, TrayState::Processing);
    tray::set_tray_status(&app, "Magpie — Transcribing...");

    // Escape only cancels during the recording phase. Once we cross into
    // transcription, release the binding so the key behaves normally again.
    unregister_escape_shortcut(&app);

    events::emit_event(&app, event_names::RECORDING_STOPPED, ());
    events::emit_event(&app, event_names::TRANSCRIPTION_STARTED, ());

    // Get the audio buffer and sample rate
    let audio_data = {
        let mut buffer = lock_or_recover(&state.audio_buffer);
        let data = buffer.clone();
        buffer.clear();
        data
    };

    let sample_rate = *lock_or_recover(&state.capture_sample_rate);

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
        // Tracks whether the transcription pipeline produced an error so the
        // overlay's auto-dismissing error pill has time to be seen by the
        // user before the overlay window hides itself.
        let mut had_error = false;

        // Get language setting and vocabulary data
        let language = {
            let settings = lock_or_recover(&state_arc.settings);
            settings.language.clone()
        };

        let (initial_prompt, vocab_replacements) = {
            let vocab = lock_or_recover(&state_arc.vocabulary);
            (vocab.get_initial_prompt_words(), vocab.get_replacements())
        };

        let prompt_ref = if initial_prompt.is_empty() {
            None
        } else {
            Some(initial_prompt.as_str())
        };

        // Clone the backend Arc out under a brief lock and release before
        // running inference. A model swap that lands after this clone
        // returns will not affect this call — the old Arc keeps the old
        // backend alive until this scope ends. Named `asr_backend` to avoid
        // shadowing inside the correction block below, which has its own
        // `backend` (the LlamaBackend).
        let asr_backend = state_arc.backend.lock().ok().and_then(|g| g.clone());
        if let Some(asr_backend) = asr_backend {
            // Resample to whatever rate the backend wants (16kHz for whisper.cpp).
            let target_rate = asr_backend.capabilities().sample_rate_hz;
            let resampled = audio::resample::resample(&audio_data, sample_rate, target_rate);

            let cancel = CancellationToken::new(); // unused for Final today; reserved
            let opts = TranscribeOptions {
                language: language.as_deref(),
                initial_prompt: prompt_ref,
                mode: TranscribeMode::Final,
            };
            let transcribe_result = asr_backend.transcribe(&resampled, &opts, &cancel);
            match transcribe_result {
                Ok(out) => {
                    let raw_text = out.text;
                    let duration_ms = out.duration_ms;
                    // Post-process
                    let (filler_words, remove_fillers) = {
                        let settings = lock_or_recover(&state_arc.settings);
                        (settings.filler_words.clone(), settings.remove_fillers)
                    };

                    let text = postprocess::postprocess(
                        &raw_text,
                        &filler_words,
                        remove_fillers,
                        &vocab_replacements,
                    );

                    // Self-correction cleanup (if enabled)
                    let text = {
                        let self_correction_enabled = {
                            let settings = lock_or_recover(&state_arc.settings);
                            settings.self_correction
                        };

                        if self_correction_enabled && !text.is_empty() {
                            tray::set_tray_status(&app_clone, "Magpie \u{2014} Cleaning up...");
                            events::emit_event(&app_clone, event_names::CORRECTION_STARTED, ());

                            let backend_guard = lock_or_recover(&state_arc.llama_backend);
                            let model_guard = lock_or_recover(&state_arc.correction_model);

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
                                        log::warn!("Correction failed, using original: {}", e);
                                        text
                                    }
                                }
                            } else {
                                log::debug!("Self-correction enabled but no model loaded");
                                text
                            }
                        } else {
                            text
                        }
                    };

                    if !text.is_empty() {
                        // Store last transcription
                        {
                            let mut last = lock_or_recover(&state_arc.last_transcription);
                            *last = text.clone();
                        }

                        // Paste into active app
                        if let Err(e) = output::paste::paste_text(&app_clone, &text) {
                            log::error!("Failed to paste text: {}", e);
                        } else {
                            log::info!("Pasted {} chars", text.len());

                            // Start correction detection if vocabulary learning is enabled
                            let vocab_learning_enabled = {
                                let settings = lock_or_recover(&state_arc.settings);
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
                            TranscriptionResult { text, duration_ms },
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
                    had_error = true;
                }
            }
        } else {
            log::warn!("Cannot transcribe: no transcription backend loaded");
            events::emit_event(
                &app_clone,
                event_names::TRANSCRIPTION_ERROR,
                TranscriptionError {
                    error: "No model loaded".to_string(),
                },
            );
            had_error = true;
        }

        state_arc.set_processing(false);
        tray::set_tray_icon(&app_clone, TrayState::Idle);
        tray::set_tray_status(&app_clone, "Magpie — Ready");
        // On error, leave the overlay visible briefly so the auto-dismissing
        // error pill (OverlayApp.vue) is actually seen. On success, hide
        // immediately — the result has already been pasted into the active app.
        if had_error {
            let app_for_hide = app_clone.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(4500)).await;
                overlay::hide_overlay(&app_for_hide);
            });
        } else {
            overlay::hide_overlay(&app_clone);
        }

        // Drain any backend reload deferred by the encoder-backfill path
        // (see lib.rs::reload_backend_after_backfill). At this point we're
        // idle: recording=false, processing=false. Schedule on the async
        // runtime so the spawn_blocking task can return immediately.
        let app_for_flush = app_clone.clone();
        let state_for_flush = state_arc.clone();
        tauri::async_runtime::spawn(async move {
            crate::flush_pending_reload(app_for_flush, state_for_flush).await;
        });
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

/// Discard the current recording without transcribing. Bound to Escape via a
/// global shortcut registered in `start_recording`. The captured audio is
/// dropped, the streaming worker is torn down, and the app returns to idle —
/// no `transcription-started` event, no paste.
#[tauri::command]
pub async fn cancel_recording(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if !state.is_recording() {
        return Err("Not recording".to_string());
    }

    // Drop the audio stream — mirrors stop_recording's first step.
    {
        let mut active = lock_or_recover(&state.active_stream);
        *active = None;
    }
    state.set_recording(false);

    // Tear down the streaming worker. partial_cancel aborts any in-flight
    // whisper.cpp call; cancel signals the loop to exit. 2s timeout bounds
    // the wait if a stuck inference somehow ignores the abort.
    let streaming_handle = state
        .streaming_handle
        .lock()
        .ok()
        .and_then(|mut g| g.take());
    if let Some(h) = streaming_handle {
        h.partial_cancel.cancel();
        h.cancel.cancel();
        let _ = tokio::time::timeout(std::time::Duration::from_millis(2000), h.join).await;
    }

    // Discard the captured audio — the whole point of "cancel" is to make
    // sure nothing reaches the transcription pipeline.
    {
        let mut buffer = lock_or_recover(&state.audio_buffer);
        buffer.clear();
    }

    // Release the global Escape binding so the key behaves normally again.
    unregister_escape_shortcut(&app);

    tray::set_tray_icon(&app, TrayState::Idle);
    tray::set_tray_status(&app, "Magpie — Ready");
    overlay::hide_overlay(&app);
    // RECORDING_STOPPED transitions the frontend to idle (no TRANSCRIPTION_STARTED
    // follows, so `processing` stays false — the overlay/popover both go quiet).
    events::emit_event(&app, event_names::RECORDING_STOPPED, ());

    log::info!("Recording cancelled by user (Escape)");
    Ok(())
}

/// Register Escape as a global shortcut that cancels the current recording.
/// Called from `start_recording`; torn down in `stop_recording` and
/// `cancel_recording` so the binding only exists while audio is being
/// captured. Outside the recording window, Escape passes through to whichever
/// app has focus.
fn register_escape_shortcut(app: &AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let escape_shortcut: Shortcut = "Escape"
        .parse()
        .map_err(|e| format!("Failed to parse Escape shortcut: {}", e))?;

    app.global_shortcut()
        .on_shortcut(escape_shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_clone.state::<Arc<AppState>>();
                    if let Err(e) = cancel_recording(app_clone.clone(), state).await {
                        // "Not recording" is the expected error if the user
                        // mashes Escape after the recording already ended;
                        // log at debug so we don't pollute the log on the
                        // common race.
                        log::debug!("Escape cancel: {}", e);
                    }
                });
            }
        })
        .map_err(|e| format!("Failed to register Escape shortcut: {}", e))?;

    log::debug!("Escape registered as cancel-recording shortcut");
    Ok(())
}

/// Idempotent teardown of the Escape cancel-recording shortcut. Silently
/// no-ops if Escape isn't currently bound.
fn unregister_escape_shortcut(app: &AppHandle) {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    if let Ok(escape_shortcut) = "Escape".parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(escape_shortcut);
    }
}

// ── App State ──────────────────────────────────────────────────────

#[tauri::command]
pub fn get_app_state(state: State<'_, Arc<AppState>>) -> AppStatePayload {
    let has_model = state.backend.lock().map(|g| g.is_some()).unwrap_or(false);
    let last_transcription = lock_or_recover(&state.last_transcription).clone();

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

    {
        let mut slot = state
            .backend
            .lock()
            .map_err(|e| format!("backend mutex poisoned: {}", e))?;
        *slot = Some(backend);
    }
    {
        let mut model_path = lock_or_recover(&state.current_model_path);
        *model_path = Some(path.to_path_buf());
    }

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

fn get_app_state_payload(state: &State<'_, Arc<AppState>>) -> AppStatePayload {
    let has_model = state.backend.lock().map(|g| g.is_some()).unwrap_or(false);
    let last_transcription = lock_or_recover(&state.last_transcription).clone();

    AppStatePayload {
        recording: state.is_recording(),
        processing: state.is_processing(),
        has_model,
        last_transcription,
    }
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

    let model_id = match state
        .settings
        .lock()
        .ok()
        .and_then(|s| s.selected_model.clone())
    {
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
    state
        .suppress_hide
        .store(true, std::sync::atomic::Ordering::SeqCst);
    crate::permissions::open_microphone_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_accessibility_settings(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state
        .suppress_hide
        .store(true, std::sync::atomic::Ordering::SeqCst);
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
    state
        .suppress_hide
        .store(true, std::sync::atomic::Ordering::SeqCst);
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

// ── Global Shortcut ────────────────────────────────────────────────

/// Re-register the global keyboard shortcut. `shortcut = None` reverts to the
/// built-in default. Returns an error if the new combination fails to parse
/// or cannot be registered (e.g. already claimed by another app).
#[tauri::command]
pub fn update_global_shortcut(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    shortcut: Option<String>,
) -> Result<(), String> {
    use crate::recording::RecordingCommand;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let new_str = shortcut
        .clone()
        .unwrap_or_else(|| crate::DEFAULT_SHORTCUT.to_string());
    let new_shortcut: Shortcut = new_str
        .parse()
        .map_err(|e| format!("Invalid shortcut '{}': {}", new_str, e))?;

    // Best-effort unregister of the previously-registered shortcut so we don't
    // leak a stale binding when the user switches combinations. Errors are
    // ignored (it might not be registered, or the binding might be stale).
    let old_str_opt = state.current_shortcut.lock().ok().and_then(|g| g.clone());
    if let Some(old_str) = old_str_opt {
        if let Ok(old) = old_str.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(old);
        }
    }

    // Clone the recording-command sender so the callback can dispatch toggles.
    let tx = match state.recording_tx.lock() {
        Ok(guard) => match guard.as_ref() {
            Some(tx) => tx.clone(),
            None => return Err("Recording channel not initialized".into()),
        },
        Err(e) => return Err(format!("recording_tx mutex poisoned: {}", e)),
    };

    app.global_shortcut()
        .on_shortcut(new_shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(e) = tx.send(RecordingCommand::Toggle) {
                    log::error!("Failed to send toggle command: {}", e);
                }
            }
        })
        .map_err(|e| format!("Failed to register shortcut: {}", e))?;

    // Persist the new value (or `None` for "use default") and update the
    // tracked string so the next re-register knows what to unregister.
    {
        let mut settings = state
            .settings
            .lock()
            .map_err(|e| format!("settings mutex poisoned: {}", e))?;
        settings.custom_shortcut = shortcut;
        if let Err(e) = settings.save() {
            log::error!("Failed to save settings after shortcut change: {}", e);
        }
    }
    if let Ok(mut current) = state.current_shortcut.lock() {
        *current = Some(new_str.clone());
    }

    log::info!("Global shortcut updated: {}", new_str);
    Ok(())
}

// ── Settings ───────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> crate::settings::UserSettings {
    lock_or_recover(&state.settings).clone()
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, Arc<AppState>>,
    mut settings: crate::settings::UserSettings,
) {
    let mut current = lock_or_recover(&state.settings);
    // Model selections are owned by download_model / select_model, not by
    // generic preference saves. A null in the incoming payload means "I
    // don't know / I didn't touch this", not "clear the selection" — so
    // preserve whatever the backend last set.
    if settings.selected_model.is_none() {
        settings.selected_model = current.selected_model.clone();
    }
    if settings.selected_correction_model.is_none() {
        settings.selected_correction_model = current.selected_correction_model.clone();
    }
    let auto_start_changed = current.auto_start != settings.auto_start;
    let new_auto_start = settings.auto_start;
    *current = settings;
    if let Err(e) = current.save() {
        log::error!("Failed to save settings: {}", e);
    }
    log::info!(
        "Settings updated; selected_model={:?}, selected_correction_model={:?}",
        current.selected_model,
        current.selected_correction_model
    );
    // Drop the settings lock before calling into SMAppService — the
    // register/unregister call can take a noticeable moment and may
    // surface a system notification on the main thread.
    drop(current);

    #[cfg(target_os = "macos")]
    if auto_start_changed {
        match crate::launch_at_login::set_enabled(new_auto_start) {
            Ok(status) => log::info!(
                "launch-at-login set to {}: status={:?}",
                new_auto_start,
                status
            ),
            Err(e) => log::error!(
                "launch-at-login set_enabled({}) failed: {}",
                new_auto_start,
                e
            ),
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (auto_start_changed, new_auto_start);
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub fn get_launch_at_login_status() -> crate::launch_at_login::LaunchAtLoginStatus {
    crate::launch_at_login::status()
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub fn get_launch_at_login_status() -> &'static str {
    "notRegistered"
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub fn open_login_items_settings() {
    crate::launch_at_login::open_login_items_settings();
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub fn open_login_items_settings() {}

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

    // Auto-load the model after download.
    // On failure, keep the file (it is likely valid) but report the error so the
    // user can retry without re-downloading.
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

    // Emit state change
    events::emit_event(
        app,
        event_names::APP_STATE_CHANGED,
        get_app_state_payload(&state),
    );

    Ok(())
}

// Vocabulary commands have been extracted to commands/vocabulary.rs.
// They are re-exported below.
