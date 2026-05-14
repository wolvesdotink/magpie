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

/// Maximum total length of the user's writing samples block (sum of all
/// samples joined with separators). Sized so the augmented system prompt
/// still fits under [`CUSTOM_PROMPT_MAX_CHARS`].
pub const WRITING_SAMPLES_MAX_CHARS: usize = 1200;

const VOICE_REFERENCE_HEADER: &str = "\n\n\
VOICE REFERENCE:\n\
The user wrote the samples below. When the cleanup rules above leave a choice between equivalent phrasings (contractions, word selection, punctuation register), prefer the option that matches this voice. Do NOT introduce words, phrases, or content from the samples that aren't already in the input. Do NOT rewrite the input to sound like the samples \u{2014} these are a tiebreaker for ambiguous choices only.\n\n\
Samples:\n---\n";

const VOICE_REFERENCE_FOOTER: &str = "\n---";

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

/// Compose a base system prompt with the user's writing samples as a "voice
/// reference" block. Pure function; no inference. Empty/whitespace-only
/// samples are dropped; if nothing remains the base prompt is returned
/// unchanged. The samples block is truncated at [`WRITING_SAMPLES_MAX_CHARS`]
/// (cut at a UTF-8 char boundary and suffixed with `…`), and if the final
/// string would still exceed [`CUSTOM_PROMPT_MAX_CHARS`] the samples block is
/// trimmed further to fit. If it can't fit at all the base is returned
/// unchanged.
pub fn augment_prompt_with_voice(base: &str, samples: &[String]) -> String {
    let cleaned: Vec<&str> = samples
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if cleaned.is_empty() {
        return base.to_string();
    }

    let joined = cleaned.join("\n---\n");
    let samples_block = truncate_at_char_boundary(&joined, WRITING_SAMPLES_MAX_CHARS);

    let overhead = base.len() + VOICE_REFERENCE_HEADER.len() + VOICE_REFERENCE_FOOTER.len();
    if overhead >= CUSTOM_PROMPT_MAX_CHARS {
        log::warn!(
            "Base correction prompt + voice-reference framing already exceeds {} chars; \
             skipping voice augmentation.",
            CUSTOM_PROMPT_MAX_CHARS
        );
        return base.to_string();
    }
    let remaining = CUSTOM_PROMPT_MAX_CHARS - overhead;
    let samples_block = truncate_at_char_boundary(&samples_block, remaining);

    let mut out = String::with_capacity(
        base.len()
            + VOICE_REFERENCE_HEADER.len()
            + samples_block.len()
            + VOICE_REFERENCE_FOOTER.len(),
    );
    out.push_str(base);
    out.push_str(VOICE_REFERENCE_HEADER);
    out.push_str(&samples_block);
    out.push_str(VOICE_REFERENCE_FOOTER);
    out
}

/// Truncate `s` to at most `max_chars` chars, appending `…` if any content was
/// dropped. Cuts at a UTF-8 char boundary so we never split a multi-byte char.
/// Returns the input unchanged when it already fits.
fn truncate_at_char_boundary(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }
    // The ellipsis is a single 3-byte UTF-8 char; reserve room for it.
    let ellipsis = "\u{2026}";
    if max_chars <= ellipsis.len() {
        // Not enough room for even the ellipsis; just return an empty slice.
        return String::new();
    }
    let budget = max_chars - ellipsis.len();
    let mut end = budget;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = String::with_capacity(end + ellipsis.len());
    out.push_str(&s[..end]);
    out.push_str(ellipsis);
    out
}

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

/// Run the correction model with an explicit system prompt (default cleanup,
/// built-in casual / formal, or user-defined custom — optionally augmented
/// with the user's writing samples via [`augment_prompt_with_voice`]). The
/// prompt is validated to be non-empty and length-capped; on validation
/// failure we return the original text rather than risk hallucination from a
/// malformed prompt.
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

    #[test]
    fn augment_with_empty_samples_returns_base_unchanged() {
        let base = "be brief";
        assert_eq!(augment_prompt_with_voice(base, &[]), base);
    }

    #[test]
    fn augment_drops_whitespace_only_samples() {
        let base = "be brief";
        let samples = vec!["   ".to_string(), "\n\t".to_string()];
        assert_eq!(augment_prompt_with_voice(base, &samples), base);
    }

    #[test]
    fn augment_appends_framing_and_samples() {
        let base = "be brief";
        let samples = vec!["I write like this.".to_string()];
        let out = augment_prompt_with_voice(base, &samples);
        assert!(out.starts_with(base));
        assert!(out.contains("VOICE REFERENCE"));
        assert!(out.contains("I write like this."));
        assert!(out.len() <= CUSTOM_PROMPT_MAX_CHARS);
    }

    #[test]
    fn augment_truncates_oversized_samples() {
        let base = "be brief";
        let huge = "a".repeat(WRITING_SAMPLES_MAX_CHARS * 4);
        let samples = vec![huge];
        let out = augment_prompt_with_voice(base, &samples);
        assert!(out.starts_with(base));
        assert!(out.len() <= CUSTOM_PROMPT_MAX_CHARS);
        // Truncation marker is present somewhere in the samples block.
        assert!(out.contains('\u{2026}'));
    }

    #[test]
    fn augment_returns_base_when_base_already_too_long() {
        let base = "x".repeat(CUSTOM_PROMPT_MAX_CHARS);
        let samples = vec!["sample".to_string()];
        let out = augment_prompt_with_voice(&base, &samples);
        assert_eq!(out, base, "no room to append voice block; keep base as-is");
    }

    #[test]
    fn augment_handles_multibyte_truncation() {
        // Each `é` is 2 bytes in UTF-8; with thousands of them the truncation
        // logic must cut on a char boundary.
        let base = "be brief";
        let big = "é".repeat(WRITING_SAMPLES_MAX_CHARS);
        let out = augment_prompt_with_voice(base, &[big]);
        assert!(
            out.is_char_boundary(out.len()),
            "result must end on a UTF-8 boundary"
        );
        assert!(out.len() <= CUSTOM_PROMPT_MAX_CHARS);
    }

    #[test]
    fn augment_joins_multiple_samples_with_separator() {
        let base = "be brief";
        let samples = vec![
            "first paragraph".to_string(),
            "second paragraph".to_string(),
        ];
        let out = augment_prompt_with_voice(base, &samples);
        assert!(out.contains("first paragraph"));
        assert!(out.contains("second paragraph"));
        assert!(out.contains("\n---\n"));
    }
}
