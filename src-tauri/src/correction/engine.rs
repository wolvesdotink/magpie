use std::collections::HashSet;
use std::num::NonZeroU32;
use std::path::Path;

use anyhow::{Context, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;

use crate::constants;

#[allow(deprecated)]
use llama_cpp_2::model::Special;

const SYSTEM_PROMPT: &str = "\
You are a dictation cleanup assistant. Your ONLY job is to remove self-corrections from dictated text.

Rules:
- When someone corrects themselves (e.g., \"no wait\", \"I mean\", \"actually\", or restates a phrase), keep ONLY the corrected version.
- When someone repeats a sentence with changes, keep ONLY the last version.
- Do NOT rephrase, reword, summarize, or improve the text in any other way.
- Do NOT add or remove punctuation beyond what the correction requires.
- Do NOT fix grammar, spelling, or style.
- If there are no corrections, return the text EXACTLY as given.

Output ONLY the cleaned text. No explanations.";

const USER_PROMPT_PREFIX: &str = "Clean up self-corrections in this dictated text:\n\n";

/// Load a correction model from a GGUF file on disk.
pub fn load_correction_model(backend: &LlamaBackend, path: &Path) -> Result<LlamaModel> {
    let start = std::time::Instant::now();
    log::info!("Loading correction model from {}", path.display());
    log::info!("GPU offload supported: {}", backend.supports_gpu_offload());
    log::info!("mmap supported: {}", backend.supports_mmap());

    // Force CPU-only inference (n_gpu_layers=0). These correction models are small
    // (0.5B-1.5B params) and run fast on CPU; no need for Metal overhead.
    let params = LlamaModelParams::default().with_n_gpu_layers(0);
    let model = LlamaModel::load_from_file(backend, path, &params)
        .map_err(|e| anyhow::anyhow!("Failed to load correction model: {:?}", e))?;

    log::info!("Correction model loaded in {:?}", start.elapsed());
    Ok(model)
}

/// Run the correction model on transcribed text to clean up self-corrections.
/// Returns the corrected text, or the original text if correction fails validation.
pub fn correct_transcription(
    backend: &LlamaBackend,
    model: &LlamaModel,
    text: &str,
) -> Result<String> {
    let start = std::time::Instant::now();

    // Build prompt using ChatML format (Qwen2.5 native format)
    let prompt = format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}{}<|im_end|>\n<|im_start|>assistant\n",
        SYSTEM_PROMPT, USER_PROMPT_PREFIX, text
    );

    // Create a fresh context for this inference
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(2048))
        .with_n_threads(constants::DEFAULT_LLM_THREADS)
        .with_n_threads_batch(constants::DEFAULT_LLM_THREADS);

    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| anyhow::anyhow!("Failed to create LLM context: {:?}", e))?;

    // Tokenize the prompt
    let tokens = model
        .str_to_token(&prompt, llama_cpp_2::model::AddBos::Always)
        .context("Failed to tokenize prompt")?;

    let n_prompt = tokens.len();
    // Cap output at ~1.1x input text tokens + 16 for safety margin
    let input_text_tokens = model
        .str_to_token(text, llama_cpp_2::model::AddBos::Never)
        .unwrap_or_default()
        .len();
    let max_new_tokens = ((input_text_tokens as f32) * 1.1 + 16.0) as usize;

    log::debug!(
        "Correction prompt: {} tokens, max new tokens: {}",
        n_prompt,
        max_new_tokens
    );

    // Feed the prompt into the model
    let mut batch = LlamaBatch::new(n_prompt.max(1), 1);
    for (i, &token) in tokens.iter().enumerate() {
        let is_last = i == n_prompt - 1;
        batch
            .add(token, i as i32, &[0], is_last)
            .context("Failed to add token to batch")?;
    }
    ctx.decode(&mut batch)
        .context("Failed to decode prompt batch")?;

    // Set up sampler: low temperature for near-deterministic output
    let mut sampler = LlamaSampler::chain_simple([LlamaSampler::temp(0.1), LlamaSampler::dist(42)]);

    // Generate tokens
    let mut output_pieces: Vec<String> = Vec::new();
    let mut n_cur = n_prompt;

    for _ in 0..max_new_tokens {
        let new_token = sampler.sample(&ctx, batch.n_tokens() - 1);

        // Check for end of generation
        if model.is_eog_token(new_token) {
            break;
        }

        // Decode token to text
        #[allow(deprecated)]
        let piece = model
            .token_to_str(new_token, Special::Plaintext)
            .unwrap_or_default();
        output_pieces.push(piece);

        // Feed the new token back
        batch.clear();
        batch
            .add(new_token, n_cur as i32, &[0], true)
            .context("Failed to add generated token to batch")?;
        ctx.decode(&mut batch)
            .context("Failed to decode generated token")?;
        n_cur += 1;
    }

    let result = output_pieces.join("").trim().to_string();
    log::info!("Correction took {:?}", start.elapsed());

    // Validate the correction
    if result.is_empty() {
        log::warn!("Correction produced empty text, using original");
        return Ok(text.to_string());
    }

    if !validate_correction(text, &result) {
        log::warn!("Correction failed validation, using original text");
        return Ok(text.to_string());
    }

    Ok(result)
}

/// Validate that the correction is reasonable and not hallucinated.
fn validate_correction(original: &str, corrected: &str) -> bool {
    // Reject if output is more than 2x the input length
    if corrected.len() as f32 > original.len() as f32 * constants::CORRECTION_MAX_OUTPUT_MULTIPLIER
    {
        log::debug!(
            "Correction rejected: output ({}) > {}x input ({})",
            corrected.len(),
            constants::CORRECTION_MAX_OUTPUT_MULTIPLIER,
            original.len()
        );
        return false;
    }

    // Reject if insufficient word overlap (hallucination check)
    let orig_words: HashSet<_> = original
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();
    let corr_words: HashSet<_> = corrected
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();

    if !corr_words.is_empty() {
        let overlap = corr_words.intersection(&orig_words).count();
        let ratio = overlap as f64 / corr_words.len() as f64;
        if ratio < constants::CORRECTION_MIN_WORD_OVERLAP {
            log::debug!(
                "Correction rejected: word overlap {:.1}% < {:.0}% threshold",
                ratio * 100.0,
                constants::CORRECTION_MIN_WORD_OVERLAP * 100.0
            );
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_correction_accepts_good_correction() {
        assert!(validate_correction(
            "I went to the store no wait the park",
            "I went to the park"
        ));
    }

    #[test]
    fn test_validate_correction_accepts_identical() {
        assert!(validate_correction("Hello world", "Hello world"));
    }

    #[test]
    fn test_validate_correction_rejects_too_long() {
        assert!(!validate_correction(
            "Short text",
            "This is a very long hallucinated output that is way longer than the original"
        ));
    }

    #[test]
    fn test_validate_correction_rejects_no_overlap() {
        assert!(!validate_correction(
            "I went to the store",
            "Completely unrelated hallucinated text about something else entirely"
        ));
    }

    #[test]
    fn test_validate_correction_accepts_subset() {
        assert!(validate_correction(
            "Send it to John. Send it to Jane.",
            "Send it to Jane."
        ));
    }
}
