use regex::Regex;

use crate::styles::{FormattingRules, PunctuationMode};
use crate::transcription::casing;
use crate::transcription::custom_rules::{self, CompiledTransform};

/// Post-process transcribed text:
///  1. filler word removal
///  2. vocabulary replacements
///  3. whitespace normalization (gated)
///  4. user-defined custom rules pipeline
///  5. punctuation transform
///  6. casing transform
///  7. trailing period strip (gated)
///
/// Defaults (`FormattingRules::default()` + empty `custom_rules`) produce
/// byte-identical output to the pre-feature behavior (Sentence casing,
/// Auto punctuation, no trailing-period strip). See the `defaults_match_legacy`
/// test.
pub fn postprocess(
    text: &str,
    filler_words: &[String],
    remove_fillers: bool,
    vocabulary_replacements: &[(String, String)],
    formatting: &FormattingRules,
    custom_rules: &[CompiledTransform],
) -> String {
    let mut result = text.to_string();

    if remove_fillers && !filler_words.is_empty() {
        result = remove_filler_words(&result, filler_words);
    }

    if !vocabulary_replacements.is_empty() {
        result = apply_vocabulary_replacements(&result, vocabulary_replacements);
    }

    if formatting.collapse_whitespace {
        result = normalize_whitespace(&result);
    }

    if !custom_rules.is_empty() {
        result = custom_rules::apply(&result, custom_rules);
    }

    result = apply_punctuation(&result, &formatting.punctuation);

    result = casing::apply(
        &result,
        formatting.casing,
        formatting.auto_capitalize_after_sentence,
    );

    if formatting.remove_trailing_period {
        result = strip_trailing_sentence_punct(&result);
    }

    result
}

/// Backwards-compatible wrapper preserving the original 4-arg signature.
/// Used by external test seams and by the streaming-preview worker if it
/// later wants the legacy defaults without resolver context.
#[allow(dead_code)]
pub fn postprocess_legacy(
    text: &str,
    filler_words: &[String],
    remove_fillers: bool,
    vocabulary_replacements: &[(String, String)],
) -> String {
    postprocess(
        text,
        filler_words,
        remove_fillers,
        vocabulary_replacements,
        &FormattingRules::default(),
        &[],
    )
}

fn remove_filler_words(text: &str, fillers: &[String]) -> String {
    let mut result = text.to_string();
    for filler in fillers {
        let pattern = format!(r"(?i)\b{}\b,?\s*", regex::escape(filler));
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, " ").to_string();
        }
    }
    result
}

fn normalize_whitespace(text: &str) -> String {
    let re = Regex::new(r"\s+").expect("invariant: literal whitespace regex compiles");
    re.replace_all(text.trim(), " ").to_string()
}

fn apply_vocabulary_replacements(text: &str, replacements: &[(String, String)]) -> String {
    let mut result = text.to_string();
    for (wrong, correct) in replacements {
        let pattern = format!(r"(?i)\b{}\b", regex::escape(wrong));
        if let Ok(re) = Regex::new(&pattern) {
            result = re
                .replace_all(&result, |caps: &regex::Captures| {
                    let matched = &caps[0];
                    apply_case_pattern(matched, correct)
                })
                .to_string();
        }
    }
    result
}

fn apply_case_pattern(source: &str, target: &str) -> String {
    if source
        .chars()
        .all(|c| !c.is_alphabetic() || c.is_uppercase())
    {
        target.to_uppercase()
    } else if source
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
    {
        let mut chars = target.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        }
    } else {
        target.to_string()
    }
}

fn apply_punctuation(text: &str, mode: &PunctuationMode) -> String {
    match mode {
        PunctuationMode::Auto => text.to_string(),
        PunctuationMode::Strip => {
            // Common punctuation set; matches what users typically want stripped.
            text.chars().filter(|c| !is_strippable_punct(*c)).collect()
        }
        PunctuationMode::SentenceOnly => text
            .chars()
            .filter(|c| matches!(c, '.' | '!' | '?') || !is_strippable_punct(*c))
            .collect(),
        PunctuationMode::Custom { chars } => {
            // Keep alphanumerics, whitespace, AND user-specified chars; strip everything else.
            text.chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace() || chars.contains(c))
                .collect()
        }
    }
}

fn is_strippable_punct(c: char) -> bool {
    matches!(
        c,
        '.' | ','
            | ';'
            | ':'
            | '!'
            | '?'
            | '"'
            | '\''
            | '`'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '—'
            | '–'
            | '…'
    )
}

fn strip_trailing_sentence_punct(text: &str) -> String {
    let trimmed = text.trim_end();
    let mut end = trimmed.len();
    for (i, c) in trimmed.char_indices().rev() {
        if matches!(c, '.' | '!' | '?') {
            end = i;
        } else {
            break;
        }
    }
    let mut out = trimmed[..end].to_string();
    out.push_str(&text[trimmed.len()..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::CasingMode;

    fn default_rules() -> FormattingRules {
        FormattingRules::default()
    }

    fn pp(
        text: &str,
        fillers: &[String],
        remove_fillers: bool,
        vocab: &[(String, String)],
    ) -> String {
        postprocess(text, fillers, remove_fillers, vocab, &default_rules(), &[])
    }

    #[test]
    fn defaults_match_legacy_filler_removal() {
        let fillers = vec!["um".to_string(), "uh".to_string()];
        let result = pp("um I think uh that this is good", &fillers, true, &[]);
        assert_eq!(result, "I think that this is good");
    }

    #[test]
    fn defaults_match_legacy_capitalization() {
        assert_eq!(pp("hello world", &[], false, &[]), "Hello world");
    }

    #[test]
    fn defaults_match_legacy_whitespace_normalization() {
        assert_eq!(pp("  hello   world  ", &[], false, &[]), "Hello world");
    }

    #[test]
    fn defaults_match_legacy_vocab_replace() {
        let vocab = vec![("Marshal".to_string(), "Marcel".to_string())];
        assert_eq!(
            pp("Hello Marshal, nice to meet you", &[], false, &vocab),
            "Hello Marcel, nice to meet you"
        );
    }

    #[test]
    fn defaults_match_legacy_vocab_preserves_case() {
        let vocab = vec![("marshal".to_string(), "Marcel".to_string())];
        assert_eq!(
            pp("hello MARSHAL and marshal", &[], false, &vocab),
            "Hello MARCEL and Marcel"
        );
    }

    #[test]
    fn defaults_match_legacy_vocab_with_fillers() {
        let fillers = vec!["um".to_string()];
        let vocab = vec![("Marshal".to_string(), "Marcel".to_string())];
        assert_eq!(
            pp("um my name is Marshal", &fillers, true, &vocab),
            "My name is Marcel"
        );
    }

    #[test]
    fn snake_case_strips_punct_and_joins() {
        let rules = FormattingRules {
            casing: CasingMode::SnakeCase,
            punctuation: PunctuationMode::Strip,
            ..Default::default()
        };
        let out = postprocess("Create user profile.", &[], false, &[], &rules, &[]);
        assert_eq!(out, "create_user_profile");
    }

    #[test]
    fn lowercase_strip_terminal_style() {
        let rules = FormattingRules {
            casing: CasingMode::Lowercase,
            punctuation: PunctuationMode::Strip,
            remove_trailing_period: true,
            ..Default::default()
        };
        let out = postprocess("ls -la ~/Desktop.", &[], false, &[], &rules, &[]);
        assert_eq!(out, "ls -la ~/desktop");
    }

    #[test]
    fn custom_punctuation_keeps_only_listed() {
        let rules = FormattingRules {
            casing: CasingMode::Preserve,
            punctuation: PunctuationMode::Custom {
                chars: vec!['.', '-'],
            },
            ..Default::default()
        };
        let out = postprocess("Hello, world! foo-bar.", &[], false, &[], &rules, &[]);
        assert_eq!(out, "Hello world foo-bar.");
    }

    #[test]
    fn sentence_only_punctuation_keeps_endings() {
        let rules = FormattingRules {
            casing: CasingMode::Preserve,
            punctuation: PunctuationMode::SentenceOnly,
            ..Default::default()
        };
        let out = postprocess("Hello, world. How are you?", &[], false, &[], &rules, &[]);
        assert_eq!(out, "Hello world. How are you?");
    }

    #[test]
    fn collapse_whitespace_off_preserves_spaces() {
        let rules = FormattingRules {
            collapse_whitespace: false,
            ..Default::default()
        };
        let out = postprocess("  hello   world  ", &[], false, &[], &rules, &[]);
        assert_eq!(out, "  Hello   world  ");
    }

    #[test]
    fn remove_trailing_period_works() {
        let rules = FormattingRules {
            remove_trailing_period: true,
            ..Default::default()
        };
        let out = postprocess("hello world.", &[], false, &[], &rules, &[]);
        assert_eq!(out, "Hello world");
    }

    #[test]
    fn auto_capitalize_after_sentence_works() {
        let rules = FormattingRules {
            auto_capitalize_after_sentence: true,
            ..Default::default()
        };
        let out = postprocess("hello world. how are you?", &[], false, &[], &rules, &[]);
        assert_eq!(out, "Hello world. How are you?");
    }

    #[test]
    fn custom_rules_run_after_vocab_before_casing() {
        use crate::styles::{TextTransform, TransformKind};
        let transforms = vec![TextTransform {
            id: "t1".into(),
            enabled: true,
            label: None,
            kind: TransformKind::Replace {
                pattern: " ".into(),
                replacement: "_".into(),
                is_regex: false,
                case_sensitive: false,
                whole_word: false,
            },
        }];
        let compiled = custom_rules::compile_all(&transforms).unwrap();
        let rules = FormattingRules {
            casing: CasingMode::Lowercase,
            punctuation: PunctuationMode::Strip,
            ..Default::default()
        };
        let out = postprocess("Hello World!", &[], false, &[], &rules, &compiled);
        assert_eq!(out, "hello_world");
    }
}
