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

use crate::constants::{whisper_threads, WHISPER_SAMPLE_RATE};

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

/// Outcome of a `WhisperBackend::self_test`. Distinguishes the specific
/// CoreML-encode failure mode from generic load/runtime errors so the
/// caller can quarantine the `.mlmodelc` instead of giving up on the
/// model entirely.
#[derive(thiserror::Error, Debug)]
pub enum SelfTestError {
    /// whisper.cpp returned `GenericError(-6)` ("failed to encode") on a
    /// trivial silent buffer. On macOS this is the diagnostic signature of
    /// a CoreML encoder that loaded but cannot run — typically a Core ML /
    /// whisper.cpp ABI mismatch on a recent macOS build.
    #[error("CoreML encoder failed self-test (whisper.cpp returned -6 on silence)")]
    CoreMLEncodeFail,
    /// Any other failure from `transcribe()`. Treated as a load-level error
    /// by the caller — the model is unusable but the encoder isn't
    /// necessarily at fault, so we don't quarantine on this path.
    #[error("self-test failed: {0}")]
    Other(String),
}

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
            .map(|stem| model_path.with_file_name(format!("{}-encoder.mlmodelc", stem)))
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

    /// Run a 2 s 220 Hz sine probe through the encode+decode pipeline as a
    /// soft startup diagnostic. Failures are logged by the caller but no
    /// longer trigger any destructive recovery (encoder quarantine etc.) —
    /// the runtime fallback infrastructure was based on a wrong diagnosis
    /// and was making things worse, so it was removed.
    ///
    /// Uses `Final` mode to mirror the dictation path the user actually
    /// invokes; that's also the path without the `abort_callback` wired in,
    /// which is what previously made the probe falsely fail on macOS 26.4 /
    /// M1 with CoreML+Metal even though real Final-mode transcribes work.
    pub fn self_test(&self) -> Result<(), SelfTestError> {
        let len = (WHISPER_SAMPLE_RATE as usize) * 2;
        let mut probe = Vec::with_capacity(len);
        for i in 0..len {
            let t = i as f32 / WHISPER_SAMPLE_RATE as f32;
            probe.push((2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.1);
        }
        let opts = TranscribeOptions {
            language: None,
            initial_prompt: None,
            mode: TranscribeMode::Final,
        };
        let cancel = CancellationToken::new();
        match self.transcribe(&probe, &opts, &cancel) {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("GenericError(-6)") || msg.contains("failed to encode") {
                    Err(SelfTestError::CoreMLEncodeFail)
                } else {
                    Err(SelfTestError::Other(msg))
                }
            }
        }
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
                params.set_n_threads(whisper_threads());
                params.set_suppress_blank(true);
                params.set_suppress_nst(true);
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
                let partial_threads = (whisper_threads() / 2).max(2);
                params.set_n_threads(partial_threads);
                params.set_suppress_blank(true);
                params.set_suppress_nst(false);
                params.set_no_timestamps(true);
                params.set_single_segment(true);
                params.set_no_context(true);
            }
        }

        // Cancellation is intentionally NOT wired into whisper.cpp's
        // abort_callback for either mode. On macOS the abort_callback poll
        // interacts badly with the CoreML/Metal encoder path, surfacing as
        // `GenericError(-6)` ("failed to encode") on real audio or — more
        // insidiously — returning empty text without any error at all.
        // Final-mode dropped the callback first (v0.1.1 worked without it).
        // PartialPreview originally kept it for stale-pass interruption,
        // but the same encoder interaction was silently killing partials:
        // worker would run, every decode would produce empty text, the
        // streaming worker's `if !out.text.is_empty()` gate dropped them,
        // and the user saw no caption pill. Removing it here matches Final.
        //
        // Caller-side cancellation is still correct: the streaming worker
        // checks `cancel.is_cancelled()` before AND after each decode
        // (`streaming.rs`), and `stop_recording` bounds the wait with a
        // 2 s `timeout` on the worker join. A stale decode in flight
        // when the user releases the hotkey costs at most one extra
        // ~200-400 ms before the worker exits — the result is dropped.
        let _ = cancel;

        state
            .full(params, audio)
            .map_err(|e| anyhow::anyhow!("Whisper inference failed: {:?}", e))?;

        // Collect text from segments. If we were aborted mid-pass, segments
        // may be empty or partial — return whatever accumulated.
        let num_segments = state.full_n_segments();
        let mut text = String::new();
        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(segment_text) = segment.to_str_lossy() {
                    text.push_str(&segment_text);
                }
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
