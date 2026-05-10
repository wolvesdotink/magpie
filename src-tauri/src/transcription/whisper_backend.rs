//! whisper.cpp implementation of `TranscriptionBackend` (via whisper-rs).
//!
//! Wraps a single loaded `WhisperContext`. `transcribe()` creates a fresh
//! `WhisperState` per call so concurrent `&self` calls don't fight over
//! decoder state — this is what lets the streaming worker's partial pass
//! and the final-on-stop pass briefly overlap without internal locking.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::constants::{DEFAULT_WHISPER_THREADS, WHISPER_SAMPLE_RATE};

use super::backend::{
    BackendCapabilities, CancellationToken, TranscribeError, TranscribeMode, TranscribeOptions,
    TranscribeOutput, TranscriptionBackend,
};

// Compile-time backend summary, derived from the whisper-rs feature flags
// active in this build. whisper.cpp also prints a runtime backend banner
// via its own logger; this gives us a single grep-able line in our logs.
#[cfg(target_os = "macos")]
const BUILT_BACKENDS: &str = "Metal + CoreML/ANE (encoder via sibling .mlmodelc if present)";
#[cfg(not(target_os = "macos"))]
const BUILT_BACKENDS: &str = "CPU";

pub struct WhisperBackend {
    ctx: WhisperContext,
}

impl WhisperBackend {
    /// Load a whisper model from disk. Synchronous — the FFI panic isolation
    /// (running this on a dedicated thread) is the caller's responsibility;
    /// see `lib.rs::try_load_last_model`.
    pub fn load(model_path: &Path) -> Result<Self> {
        // CoreML escape hatch. whisper.cpp's CoreML init is opt-in at
        // runtime via `WhisperContextParameters::use_gpu` indirectly; the
        // `coreml` cargo feature compiles in the path but a missing
        // `.mlmodelc` should still let inference fall back to Metal. If a
        // user hits the encoder-failure mode anyway (whisper.cpp returning
        // GenericError(-6)), setting `MAGPIE_DISABLE_COREML=1` skips the
        // ANE encoder load attempt by clearing `use_gpu`. This is a safety
        // valve, not a recommended config — it disables Metal too. See
        // README for the recommended `MAGPIE_DISABLE_COREML` workflow.
        let disable_coreml = std::env::var("MAGPIE_DISABLE_COREML")
            .map(|v| !v.is_empty() && v != "0")
            .unwrap_or(false);

        // Surface CoreML encoder presence in the load log so a future
        // GenericError(-6) is easy to diagnose. The encoder lives next to
        // the GGML file as `<stem>-encoder.mlmodelc` (see downloader.rs).
        let coreml_present = model_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|stem| {
                model_path.with_file_name(format!("{}-encoder.mlmodelc", stem))
            })
            .map(|p| p.exists())
            .unwrap_or(false);
        let coreml_status = if disable_coreml {
            "disabled via MAGPIE_DISABLE_COREML"
        } else if coreml_present {
            "encoder present"
        } else {
            "encoder MISSING — may cause GenericError(-6) until backfill completes"
        };
        log::info!(
            "Loading whisper model from: {} (backend: {}, CoreML: {})",
            model_path.display(),
            BUILT_BACKENDS,
            coreml_status,
        );
        let start = Instant::now();

        let mut params = WhisperContextParameters::default();
        if disable_coreml {
            // Whisper-rs 0.13 doesn't expose a dedicated CoreML toggle — the
            // closest knob is `use_gpu`, which gates BOTH Metal and the
            // CoreML path. Disabling falls back to CPU inference; slow but
            // always works. Documented as the last-resort recovery in the
            // README.
            log::warn!("MAGPIE_DISABLE_COREML set — disabling GPU acceleration (CPU only)");
            params.use_gpu = false;
        }
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().context("Invalid model path")?,
            params,
        )
        .map_err(|e| anyhow::anyhow!("Failed to load whisper model: {:?}", e))?;

        log::info!("Model loaded in {:?}", start.elapsed());
        Ok(Self { ctx })
    }
}

impl TranscriptionBackend for WhisperBackend {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            sample_rate_hz: WHISPER_SAMPLE_RATE,
            supports_initial_prompt: true,
            supports_partial_decode: true,
        }
    }

    fn name(&self) -> &'static str {
        "whisper.cpp"
    }

    fn transcribe(
        &self,
        audio: &[f32],
        opts: &TranscribeOptions<'_>,
        cancel: &CancellationToken,
    ) -> Result<TranscribeOutput, TranscribeError> {
        let start = Instant::now();

        // Per-call state. Storing nothing on Self is what makes &self
        // transcribe safe to call concurrently.
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create whisper state: {:?}", e))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Common config across both modes.
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_translate(false);
        params.set_language(opts.language);

        match opts.mode {
            TranscribeMode::Final => {
                params.set_n_threads(DEFAULT_WHISPER_THREADS);
                params.set_suppress_blank(true);
                params.set_suppress_non_speech_tokens(true);
                params.set_no_timestamps(true);
                params.set_single_segment(false);
                if let Some(prompt) = opts.initial_prompt {
                    if !prompt.is_empty() {
                        log::info!(
                            "Using initial prompt for vocabulary biasing ({} chars)",
                            prompt.len()
                        );
                        params.set_initial_prompt(prompt);
                    }
                }
            }
            TranscribeMode::PartialPreview => {
                // Cheap params: cap threads so we don't starve the cpal callback,
                // single-segment so whisper.cpp doesn't try to chunk a 30s window
                // multiple times, no_context so a stale partial doesn't bias the
                // next one. Skip the initial_prompt — re-injecting vocab on every
                // 1.5s tick wastes prompt tokens for no quality gain in a preview.
                let partial_threads = (DEFAULT_WHISPER_THREADS / 2).max(2);
                params.set_n_threads(partial_threads);
                params.set_suppress_blank(true);
                params.set_suppress_non_speech_tokens(false);
                params.set_no_timestamps(true);
                params.set_single_segment(true);
                params.set_no_context(true);
            }
        }

        // Wire cancellation into whisper.cpp's abort_callback. The callback
        // fires periodically inside the inference loop; returning true makes
        // whisper.cpp bail out cleanly. The token is cloned so it outlives
        // this call frame — the closure box is owned by whisper-rs internals.
        let cancel_token = cancel.clone();
        params.set_abort_callback_safe(move || cancel_token.is_cancelled());

        state
            .full(params, audio)
            .map_err(|e| anyhow::anyhow!("Whisper inference failed: {:?}", e))?;

        // Collect text from segments. If we were aborted mid-pass, segments
        // may be empty or partial — return whatever accumulated.
        let num_segments = state
            .full_n_segments()
            .map_err(|e| anyhow::anyhow!("Failed to get segment count: {:?}", e))?;
        let mut text = String::new();
        for i in 0..num_segments {
            if let Ok(segment_text) = state.full_get_segment_text(i) {
                text.push_str(&segment_text);
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        if matches!(opts.mode, TranscribeMode::Final) {
            log::info!(
                "Transcription complete in {}ms: \"{}\"",
                duration_ms,
                text.trim()
            );
        }

        Ok(TranscribeOutput {
            text: text.trim().to_string(),
            duration_ms,
        })
    }
}
