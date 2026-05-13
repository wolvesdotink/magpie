use std::collections::HashSet;
use std::num::NonZeroU32;
use std::path::Path;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;

use crate::constants;
use crate::correction::{CorrectionError, Result};

/// Map any `Display`-able llama_cpp_2 error into `CorrectionError::Inference`
/// with a contextual prefix.
fn inference_err<E: std::fmt::Display>(prefix: &'static str) -> impl FnOnce(E) -> CorrectionError {
    move |e| CorrectionError::Inference(format!("{prefix}: {e}"))
}

#[allow(deprecated)]
use llama_cpp_2::model::Special;

/// Maximum length of a user-supplied custom correction prompt.
pub const CUSTOM_PROMPT_MAX_CHARS: usize = 2048;

pub const SYSTEM_PROMPT: &str = "\
You are a dictation cleanup assistant. Your ONLY job is to remove self-corrections from dictated text.

Rules:
- When someone corrects themselves (e.g., \"no wait\", \"I mean\", \"actually\", or restates a phrase), keep ONLY the corrected version.
- When someone repeats a sentence with changes, keep ONLY the last version.
- Do NOT rephrase, reword, summarize, or improve the text in any other way.
- Do NOT add or remove punctuation beyond what the correction requires.
- Do NOT fix grammar, spelling, or style.
- If there are no corrections, return the text EXACTLY as given.

Output ONLY the cleaned text. No explanations.";

pub const CASUAL_SYSTEM_PROMPT: &str = "\
You are a dictation cleanup assistant for casual messages (Slack, Discord, chat).

Rules:
- Remove self-corrections: when someone restates a phrase or says \"I mean\" / \"no wait\" / \"actually\", keep ONLY the corrected version.
- Keep contractions (don't, can't, won't, etc.) and casual phrasing as-is.
- Light punctuation normalization is OK (e.g., adding a missing comma in a long run-on); do NOT rewrite for style.
- Do NOT fix grammar, spelling, or formality.
- If there are no self-corrections, return the text EXACTLY as given.

Output ONLY the cleaned text. No explanations.";

pub const FORMAL_SYSTEM_PROMPT: &str = "\
You are a dictation cleanup assistant for formal writing (email, documents).

Rules:
- Remove self-corrections: when someone restates a phrase or says \"I mean\" / \"no wait\" / \"actually\", keep ONLY the corrected version.
- Light clarity edits are allowed: fix obvious punctuation, capitalize sentence starts, expand obviously incorrect contractions ONLY if doing so is more natural in formal prose.
- Do NOT rewrite sentences, change vocabulary choice, or alter the author's voice.
- If there are no self-corrections or obvious cleanups, return the text EXACTLY as given.

Output ONLY the cleaned text. No explanations.";

const USER_PROMPT_PREFIX: &str = "Clean up self-corrections in this dictated text:\n\n";

/// Load a correction model from a GGUF file on disk.
pub fn load_correction_model(backend: &LlamaBackend, path: &Path) -> Result<LlamaModel> {
    let start = std::time::Instant::now();
    log::info!("Loading correction model from {}", path.display());
    log::info!("GPU offload supported: {}", backend.supports_gpu_offload());
    log::info!("mmap supported: {}", backend.supports_mmap());

    let params = LlamaModelParams::default().with_n_gpu_layers(0);
    let model = LlamaModel::load_from_file(backend, path, &params).map_err(|e| {
        CorrectionError::ModelLoad {
            path: path.to_path_buf(),
            message: format!("{e:?}"),
        }
    })?;

    log::info!("Correction model loaded in {:?}", start.elapsed());
    Ok(model)
}

/// Run the correction model on transcribed text, with the default system prompt.
pub fn correct_transcription(
    backend: &LlamaBackend,
    model: &LlamaModel,
    text: &str,
) -> Result<String> {
    correct_transcription_with_prompt(backend, model, text, SYSTEM_PROMPT)
}

/// Run the correction model with an explicit system prompt (built-in casual /
/// formal / user-defined custom). The prompt is validated to be non-empty and
/// length-capped; on validation failure we return the original text rather
/// than risk hallucination from a malformed prompt.
pub fn correct_transcription_with_prompt(
    backend: &LlamaBackend,
    model: &LlamaModel,
    text: &str,
    system_prompt: &str,
) -> Result<String> {
    let start = std::time::Instant::now();

    let trimmed = system_prompt.trim();
    if trimmed.is_empty() {
        log::warn!("Empty correction system prompt; using original text");
        return Ok(text.to_string());
    }
    if trimmed.len() > CUSTOM_PROMPT_MAX_CHARS {
        log::warn!(
            "Correction system prompt exceeds {} chars; using original text",
            CUSTOM_PROMPT_MAX_CHARS
        );
        return Ok(text.to_string());
    }

    let prompt = format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}{}<|im_end|>\n<|im_start|>assistant\n",
        trimmed, USER_PROMPT_PREFIX, text
    );

    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(2048))
        .with_n_threads(constants::DEFAULT_LLM_THREADS)
        .with_n_threads_batch(constants::DEFAULT_LLM_THREADS);

    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(inference_err("create LLM context"))?;

    let tokens = model
        .str_to_token(&prompt, llama_cpp_2::model::AddBos::Always)
        .map_err(inference_err("tokenize prompt"))?;

    let n_prompt = tokens.len();
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

    let mut batch = LlamaBatch::new(n_prompt.max(1), 1);
    for (i, &token) in tokens.iter().enumerate() {
        let is_last = i == n_prompt - 1;
        batch
            .add(token, i as i32, &[0], is_last)
            .map_err(inference_err("add token to batch"))?;
    }
    ctx.decode(&mut batch)
        .map_err(inference_err("decode prompt batch"))?;

    let mut sampler = LlamaSampler::chain_simple([LlamaSampler::temp(0.1), LlamaSampler::dist(42)]);

    let mut output_pieces: Vec<String> = Vec::new();
    let mut n_cur = n_prompt;

    for _ in 0..max_new_tokens {
        let new_token = sampler.sample(&ctx, batch.n_tokens() - 1);

        if model.is_eog_token(new_token) {
            break;
        }

        #[allow(deprecated)]
        let piece = model
            .token_to_str(new_token, Special::Plaintext)
            .unwrap_or_default();
        output_pieces.push(piece);

        batch.clear();
        batch
            .add(new_token, n_cur as i32, &[0], true)
            .map_err(inference_err("add generated token to batch"))?;
        ctx.decode(&mut batch)
            .map_err(inference_err("decode generated token"))?;
        n_cur += 1;
    }

    let result = output_pieces.join("").trim().to_string();
    log::info!("Correction took {:?}", start.elapsed());

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

fn validate_correction(original: &str, corrected: &str) -> bool {
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
