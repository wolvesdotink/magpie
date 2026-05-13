//! Recording-control Tauri commands and their helpers.
//!
//! Four user-facing commands:
//!   - start_recording — opens the cpal stream, kicks off the streaming-
//!     preview worker (if enabled) and the amplitude emitter, registers
//!     Escape as a cancel shortcut.
//!   - stop_recording — closes the stream, runs the final transcription
//!     on a spawn_blocking task, applies postprocess + self-correction,
//!     pastes via the output module, kicks off correction detection.
//!   - toggle_recording — start-if-stopped / stop-if-running.
//!   - cancel_recording — Escape-bound; tears down without transcribing.
//!
//! Two private helpers (`register_escape_shortcut`, `unregister_escape_shortcut`)
//! manage the global Escape binding's lifetime so it only exists while a
//! recording is in flight.

use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::audio;
use crate::command_error::CommandError;
use crate::correction;
use crate::correction_detector;
use crate::events::{
    self, event_names, AudioAmplitudePayload, TranscriptionError, TranscriptionResult,
};
use crate::frontmost_app;
use crate::output;
use crate::overlay;
use crate::resolver;
use crate::state::{lock_or_recover, AppState};
use crate::styles::CorrectionOverride;
use crate::transcription::backend::{CancellationToken, TranscribeMode, TranscribeOptions};
use crate::transcription::postprocess;
use crate::transcription::streaming;
use crate::tray::{self, TrayState};

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), CommandError> {
    if state.is_recording() {
        return Err(CommandError::InvalidArgument {
            message: "Already recording".into(),
        });
    }

    if state.is_processing() {
        return Err(CommandError::InvalidArgument {
            message: "Currently processing".into(),
        });
    }

    // Pre-flight the microphone TCC grant. cpal will otherwise open a stream
    // that delivers silent samples (or fails with an opaque device error,
    // depending on macOS version), and the user gets a transcription full
    // of nothing without knowing why. Returning a typed `PermissionDenied`
    // here lets the frontend dispatch to the "Grant microphone access" CTA.
    if crate::permissions::microphone_authorization_status()
        != crate::permissions::MicrophoneAuthStatus::Authorized
    {
        return Err(CommandError::PermissionDenied {
            permission: "microphone".into(),
        });
    }

    let state_arc = state.inner().clone();

    // Capture frontmost app BEFORE the audio stream opens so we can attribute
    // this recording's profile + vocabulary learning to wherever the user's
    // intent began (Magpie's own settings window does not steal focus during
    // start_recording, so this is the right moment).
    {
        let mut current_app = lock_or_recover(&state.current_recording_app);
        *current_app = frontmost_app::detect();
        if let Some(ref app) = *current_app {
            log::info!(
                "Recording target: {} ({})",
                app.name,
                app.bundle_id
            );
        } else {
            log::debug!("No frontmost app detected at recording start");
        }
    }

    let (stream, sample_rate) = audio::capture::start_recording(&state_arc)?;

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
    // every ~300 ms and emit PARTIAL_TRANSCRIPTION events the overlay can
    // render as a live caption. Skip if no backend is loaded — there's
    // nothing to decode against — or if the user has disabled the live
    // preview in Settings (default off; final-on-stop is unaffected).
    let backend_present = lock_or_recover(&state.backend).is_some();
    let streaming_enabled = lock_or_recover(&state.settings).streaming_preview;
    if backend_present && streaming_enabled {
        let handle = streaming::spawn_streaming_worker(app.clone(), state_arc.clone());
        *lock_or_recover(&state.streaming_handle) = Some(handle);
    } else if !backend_present {
        log::info!("Streaming worker not started: no backend loaded");
    } else {
        log::info!("Streaming worker not started: live preview disabled in settings");
    }

    // Spawn the amplitude emitter on the tokio runtime so the wake-ups are
    // scheduled by the runtime rather than by `std::thread::sleep`. The task
    // exits naturally when `is_recording()` flips back to false.
    {
        let emitter_app = app.clone();
        let emitter_state = state_arc.clone();
        tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_millis(50));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            while emitter_state.is_recording() {
                ticker.tick().await;
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
            }

            // Emit a final zero so the frontend can animate bars down to rest.
            events::emit_event(
                &emitter_app,
                event_names::AUDIO_AMPLITUDE,
                AudioAmplitudePayload { amplitude: 0.0 },
            );

            log::debug!("Amplitude emitter task exiting");
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
pub async fn stop_recording(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), CommandError> {
    if !state.is_recording() {
        return Err(CommandError::InvalidArgument {
            message: "Not recording".into(),
        });
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
    let streaming_handle = lock_or_recover(&state.streaming_handle).take();
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

    // Snapshot the audio under a brief lock, then clear so the next
    // recording starts clean. The ring buffer may have evicted older
    // samples if the user dictated past MAX_BUFFER_SAMPLES — the
    // snapshot reflects the retained window only.
    let audio_data = {
        let mut buffer = lock_or_recover(&state.audio_buffer);
        let data = buffer.snapshot();
        if buffer.has_overflowed() {
            log::warn!(
                "Audio ring buffer overflowed during this recording — \
                 final transcription is from the most recent {} samples only",
                data.len()
            );
        }
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

        // Get language setting (needed before resolver call, which doesn't
        // touch language).
        let language = {
            let settings = lock_or_recover(&state_arc.settings);
            settings.language.clone()
        };

        // Resolve effective configuration (profile match → style + vocab merge
        // + compiled custom rules + correction override).
        let effective = match resolver::resolve(
            &state_arc.current_recording_app,
            &state_arc.profiles,
            &state_arc.styles,
            &state_arc.vocabulary,
            &state_arc.settings,
        ) {
            Ok(e) => e,
            Err(e) => {
                log::error!("Effective resolution failed: {}", e);
                events::emit_event(
                    &app_clone,
                    event_names::TRANSCRIPTION_ERROR,
                    TranscriptionError {
                        error: format!("Style configuration error: {}", e),
                    },
                );
                state_arc.set_processing(false);
                tray::set_tray_icon(&app_clone, TrayState::Idle);
                tray::set_tray_status(&app_clone, "Magpie — Ready");
                overlay::hide_overlay(&app_clone);
                return;
            }
        };

        let initial_prompt = effective.initial_prompt;
        let vocab_replacements = effective.vocab_replacements;
        let formatting = effective.formatting;
        let compiled_transforms = effective.compiled_transforms;
        let correction_override = effective.correction;
        let resolved_remove_fillers = effective.remove_fillers;
        let vocab_learning_enabled = effective.vocab_learning_enabled;

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
        let asr_backend = lock_or_recover(&state_arc.backend).clone();
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
                    // Post-process with style-resolved formatting + custom rules.
                    let filler_words = {
                        let settings = lock_or_recover(&state_arc.settings);
                        settings.filler_words.clone()
                    };

                    let text = postprocess::postprocess(
                        &raw_text,
                        &filler_words,
                        resolved_remove_fillers,
                        &vocab_replacements,
                        &formatting,
                        &compiled_transforms,
                    );

                    // Self-correction cleanup. Honors the style's CorrectionOverride:
                    //   Inherit  → use global self_correction toggle + default prompt
                    //   Disabled → skip entirely
                    //   Casual/Formal/Custom → run with overridden prompt (still gated
                    //                          on global toggle so a user with global
                    //                          correction off isn't surprised)
                    let text = {
                        let self_correction_global = {
                            let settings = lock_or_recover(&state_arc.settings);
                            settings.self_correction
                        };

                        let (should_correct, custom_prompt) = match &correction_override {
                            CorrectionOverride::Disabled => (false, None),
                            CorrectionOverride::Inherit => (self_correction_global, None),
                            CorrectionOverride::Casual => (
                                self_correction_global,
                                Some(correction::engine::CASUAL_SYSTEM_PROMPT),
                            ),
                            CorrectionOverride::Formal => (
                                self_correction_global,
                                Some(correction::engine::FORMAL_SYSTEM_PROMPT),
                            ),
                            CorrectionOverride::Custom { prompt } => {
                                (self_correction_global, Some(prompt.as_str()))
                            }
                        };

                        if should_correct && !text.is_empty() {
                            tray::set_tray_status(&app_clone, "Magpie \u{2014} Cleaning up...");
                            events::emit_event(&app_clone, event_names::CORRECTION_STARTED, ());

                            let backend_guard = lock_or_recover(&state_arc.llama_backend);
                            let model_guard = lock_or_recover(&state_arc.correction_model);

                            if let (Some(ref backend), Some(ref model)) =
                                (&*backend_guard, &*model_guard)
                            {
                                let result = match custom_prompt {
                                    Some(p) => correction::engine::correct_transcription_with_prompt(
                                        backend, model, &text, p,
                                    ),
                                    None => correction::engine::correct_transcription(
                                        backend, model, &text,
                                    ),
                                };
                                match result {
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
                                log::debug!("Self-correction requested but no model loaded");
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

                        // Append to on-disk history (best-effort; never blocks
                        // the paste). Two scopes so neither guard is held
                        // across the next lock or the paste call — keeps the
                        // settings → history ordering on rank 1 → rank 7.5.
                        let history_cap = {
                            let s = lock_or_recover(&state_arc.settings);
                            s.history_max_entries as usize
                        };
                        {
                            let mut hist = lock_or_recover(&state_arc.history);
                            hist.push(text.clone(), duration_ms, history_cap);
                            if let Err(e) = hist.save() {
                                log::warn!(
                                    "Failed to persist transcription history: {}",
                                    e
                                );
                            }
                        }
                        events::emit_event(
                            &app_clone,
                            event_names::HISTORY_ENTRY_ADDED,
                            (),
                        );

                        // Paste into active app
                        if let Err(e) = output::paste::paste_text(&app_clone, &text) {
                            log::error!("Failed to paste text: {}", e);
                        } else {
                            log::info!("Pasted {} chars", text.len());

                            // Start correction detection if vocabulary learning is enabled
                            // (style/profile override applied during resolution).
                            if vocab_learning_enabled {
                                let captured_app = {
                                    let lock = lock_or_recover(&state_arc.current_recording_app);
                                    lock.clone()
                                };
                                correction_detector::start_detection(
                                    text.clone(),
                                    state_arc.clone(),
                                    app_clone.clone(),
                                    captured_app,
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
) -> Result<(), CommandError> {
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
) -> Result<(), CommandError> {
    if !state.is_recording() {
        return Err(CommandError::InvalidArgument {
            message: "Not recording".into(),
        });
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
    let streaming_handle = lock_or_recover(&state.streaming_handle).take();
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
fn register_escape_shortcut(app: &AppHandle) -> Result<(), CommandError> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let escape_shortcut: Shortcut =
        "Escape"
            .parse()
            .map_err(|e| CommandError::InvalidArgument {
                message: format!("Failed to parse Escape shortcut: {e}"),
            })?;

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
        .map_err(|e| CommandError::other(format!("Failed to register Escape shortcut: {e}")))?;

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
