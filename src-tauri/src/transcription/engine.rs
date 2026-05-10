use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::constants::DEFAULT_WHISPER_THREADS;

// Compile-time backend summary, derived from the whisper-rs feature flags
// active in this build. whisper.cpp also prints a runtime backend banner
// via its own logger; this gives us a single grep-able line in our logs.
#[cfg(target_os = "macos")]
const BUILT_BACKENDS: &str = "Metal + CoreML/ANE (encoder via sibling .mlmodelc if present)";
#[cfg(not(target_os = "macos"))]
const BUILT_BACKENDS: &str = "CPU";

/// Load a whisper model from disk
pub fn load_model(model_path: &Path) -> Result<WhisperContext> {
    log::info!(
        "Loading whisper model from: {} (backend: {})",
        model_path.display(),
        BUILT_BACKENDS,
    );
    let start = Instant::now();

    let params = WhisperContextParameters::default();
    let ctx = WhisperContext::new_with_params(
        model_path.to_str().context("Invalid model path")?,
        params,
    )
    .map_err(|e| anyhow::anyhow!("Failed to load whisper model: {:?}", e))?;

    log::info!("Model loaded in {:?}", start.elapsed());
    Ok(ctx)
}

/// Transcribe audio samples (16kHz mono f32) using the given whisper context.
/// Returns the transcribed text and duration in milliseconds.
///
/// If `initial_prompt` is provided, it biases Whisper toward recognizing
/// specific words (e.g. learned vocabulary corrections).
pub fn transcribe(
    ctx: &WhisperContext,
    audio: &[f32],
    language: Option<&str>,
    initial_prompt: Option<&str>,
) -> Result<(String, u64)> {
    let start = Instant::now();

    let mut state = ctx
        .create_state()
        .map_err(|e| anyhow::anyhow!("Failed to create whisper state: {:?}", e))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    // Configure params
    params.set_n_threads(DEFAULT_WHISPER_THREADS);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_suppress_non_speech_tokens(true);
    params.set_no_timestamps(true);
    params.set_single_segment(false);
    params.set_translate(false);

    // None = auto-detect, Some("en") = English, etc.
    params.set_language(language);

    // Inject vocabulary prompt to bias recognition toward learned words
    if let Some(prompt) = initial_prompt {
        if !prompt.is_empty() {
            log::info!("Using initial prompt for vocabulary biasing ({} chars)", prompt.len());
            params.set_initial_prompt(prompt);
        }
    }

    // Run inference
    state
        .full(params, audio)
        .map_err(|e| anyhow::anyhow!("Whisper inference failed: {:?}", e))?;

    // Collect text from all segments
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
    log::info!(
        "Transcription complete in {}ms: \"{}\"",
        duration_ms,
        text.trim()
    );

    Ok((text.trim().to_string(), duration_ms))
}
