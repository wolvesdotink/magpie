//! Built-in styles seeded on first launch.
//!
//! IDs are deterministic constants so profile FKs always resolve and "Reset
//! built-in presets" can locate them after the user has edited or deleted
//! anything else.

use chrono::Utc;

use super::{CasingMode, CorrectionOverride, FormattingRules, PunctuationMode, Style};

pub const BUILTIN_DEFAULT_ID: &str = "builtin-default";
pub const BUILTIN_CASUAL_ID: &str = "builtin-casual";
pub const BUILTIN_FORMAL_ID: &str = "builtin-formal";
pub const BUILTIN_CODE_IDENTIFIER_ID: &str = "builtin-code-identifier";
pub const BUILTIN_CODE_SNAKE_ID: &str = "builtin-code-snake";
pub const BUILTIN_PLAIN_LOWER_ID: &str = "builtin-plain-lower";

#[allow(dead_code)]
pub const BUILTIN_IDS: &[&str] = &[
    BUILTIN_DEFAULT_ID,
    BUILTIN_CASUAL_ID,
    BUILTIN_FORMAL_ID,
    BUILTIN_CODE_IDENTIFIER_ID,
    BUILTIN_CODE_SNAKE_ID,
    BUILTIN_PLAIN_LOWER_ID,
];

pub fn builtin_styles() -> Vec<Style> {
    let now = Utc::now().to_rfc3339();
    vec![
        Style {
            id: BUILTIN_DEFAULT_ID.into(),
            name: "Default".into(),
            description: Some("Current Magpie behavior — fallback when no profile matches".into()),
            builtin: true,
            formatting: FormattingRules {
                casing: CasingMode::Sentence,
                punctuation: PunctuationMode::Auto,
                remove_trailing_period: false,
                auto_capitalize_after_sentence: false,
                collapse_whitespace: true,
            },
            correction: CorrectionOverride::Inherit,
            custom_rules: vec![],
            filler_override: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        Style {
            id: BUILTIN_CASUAL_ID.into(),
            name: "Casual".into(),
            description: Some("Light touch; keeps contractions and informal register".into()),
            builtin: true,
            formatting: FormattingRules {
                casing: CasingMode::Sentence,
                punctuation: PunctuationMode::Auto,
                remove_trailing_period: false,
                auto_capitalize_after_sentence: true,
                collapse_whitespace: true,
            },
            correction: CorrectionOverride::Casual,
            custom_rules: vec![],
            filler_override: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        Style {
            id: BUILTIN_FORMAL_ID.into(),
            name: "Formal".into(),
            description: Some("Polished sentences; allows light clarity edits".into()),
            builtin: true,
            formatting: FormattingRules {
                casing: CasingMode::Sentence,
                punctuation: PunctuationMode::Auto,
                remove_trailing_period: false,
                auto_capitalize_after_sentence: true,
                collapse_whitespace: true,
            },
            correction: CorrectionOverride::Formal,
            custom_rules: vec![],
            filler_override: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        Style {
            id: BUILTIN_CODE_IDENTIFIER_ID.into(),
            name: "Code Identifier".into(),
            description: Some(
                "For editors — preserves whisper casing, strips punctuation".into(),
            ),
            builtin: true,
            formatting: FormattingRules {
                casing: CasingMode::Preserve,
                punctuation: PunctuationMode::Strip,
                remove_trailing_period: true,
                auto_capitalize_after_sentence: false,
                collapse_whitespace: true,
            },
            correction: CorrectionOverride::Disabled,
            custom_rules: vec![],
            filler_override: Some(false),
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        Style {
            id: BUILTIN_CODE_SNAKE_ID.into(),
            name: "Code Snake Case".into(),
            description: Some(
                "For dictating identifiers like `create_user_profile`".into(),
            ),
            builtin: true,
            formatting: FormattingRules {
                casing: CasingMode::SnakeCase,
                punctuation: PunctuationMode::Strip,
                remove_trailing_period: true,
                auto_capitalize_after_sentence: false,
                collapse_whitespace: true,
            },
            correction: CorrectionOverride::Disabled,
            custom_rules: vec![],
            filler_override: Some(false),
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        Style {
            id: BUILTIN_PLAIN_LOWER_ID.into(),
            name: "Plain Lowercase".into(),
            description: Some("For terminals — strict lowercase, no punctuation".into()),
            builtin: true,
            formatting: FormattingRules {
                casing: CasingMode::Lowercase,
                punctuation: PunctuationMode::Strip,
                remove_trailing_period: true,
                auto_capitalize_after_sentence: false,
                collapse_whitespace: true,
            },
            correction: CorrectionOverride::Disabled,
            custom_rules: vec![],
            filler_override: Some(false),
            created_at: now.clone(),
            updated_at: now,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<&str> = BUILTIN_IDS.to_vec();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), BUILTIN_IDS.len(), "builtin ids must be unique");
    }

    #[test]
    fn presets_match_ids() {
        let presets = builtin_styles();
        for id in BUILTIN_IDS {
            assert!(presets.iter().any(|s| &s.id == id), "missing preset: {id}");
        }
        for preset in presets {
            assert!(preset.builtin, "preset {} must have builtin=true", preset.id);
        }
    }
}
