//! Resolves the "effective" transcription configuration at recording time.
//!
//! Given the captured frontmost app + the current profiles, styles, and global
//! vocabulary + settings, the resolver produces a single `EffectiveResolution`
//! that drives the rest of the transcription pipeline (initial_prompt, vocab
//! replacements, formatting rules, compiled custom-rule pipeline, correction
//! override, learning target).

use parking_lot::Mutex;

use crate::frontmost_app::FrontmostApp;
use crate::profiles::ProfilesStore;
use crate::settings::UserSettings;
use crate::state::lock_or_recover;
use crate::styles::{
    presets as style_presets, CorrectionOverride, FormattingRules, Style, StylesStore,
};
use crate::transcription::custom_rules::{self, CompiledTransform};
use crate::vocabulary::{Vocabulary, VocabularyEntry};

/// Maximum character budget for the Whisper initial_prompt. Mirrors
/// `vocabulary::INITIAL_PROMPT_CHAR_BUDGET` so the unioned list stays under
/// whisper's token cap (~224 tokens at ~4 chars/token).
const INITIAL_PROMPT_CHAR_BUDGET: usize = 800;

/// Snapshot of everything the transcription pipeline needs after profile
/// + style + vocab resolution. Cheap to move; owns its data.
#[derive(Debug)]
pub struct EffectiveResolution {
    pub initial_prompt: String,
    pub vocab_replacements: Vec<(String, String)>,
    pub formatting: FormattingRules,
    pub compiled_transforms: Vec<CompiledTransform>,
    pub correction: CorrectionOverride,
    pub remove_fillers: bool,
    pub vocab_learning_enabled: bool,
    /// Whether learned vocabulary should attribute to a profile, and which
    /// one. `None` means attribute to global vocabulary. Reserved for future
    /// auto-learning routing logic that lives outside the resolver.
    #[allow(dead_code)]
    pub learning_target_profile_id: Option<String>,
    /// Optional id of the matched profile (for UI feedback).
    #[allow(dead_code)]
    pub matched_profile_id: Option<String>,
    /// Optional id of the style that was applied (built-in or user).
    #[allow(dead_code)]
    pub matched_style_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("custom rules failed to compile: {0}")]
    BadTransform(#[from] custom_rules::CompileError),
}

/// Resolve the effective configuration for a recording.
///
/// All mutex acquisitions follow the documented lock order
/// (settings → styles → profiles → vocabulary).
pub fn resolve(
    current_app: &Mutex<Option<FrontmostApp>>,
    profiles: &Mutex<ProfilesStore>,
    styles: &Mutex<StylesStore>,
    vocabulary: &Mutex<Vocabulary>,
    settings: &Mutex<UserSettings>,
) -> Result<EffectiveResolution, ResolveError> {
    let (filler_global, remove_fillers_global, vocab_learning_global) = {
        let s = lock_or_recover(settings);
        (
            s.filler_words.clone(),
            s.remove_fillers,
            s.vocabulary_learning,
        )
    };
    // Silence unused warning when feature evolution adds filler tweaks here.
    let _ = filler_global;

    let app_snapshot = lock_or_recover(current_app).clone();
    let profile = app_snapshot.as_ref().and_then(|app| {
        let p = lock_or_recover(profiles);
        p.find_by_bundle(&app.bundle_id).cloned()
    });

    // Determine style: profile's style → fallback to builtin-default.
    let style: Style = {
        let s = lock_or_recover(styles);
        match &profile {
            Some(p) => s.get_or_default(&p.style_id).clone(),
            None => s
                .get(style_presets::BUILTIN_DEFAULT_ID)
                .cloned()
                .unwrap_or_else(|| {
                    // Shouldn't happen — builtin-default is seeded — but degrade gracefully.
                    style_presets::builtin_styles()
                        .into_iter()
                        .find(|s| s.id == style_presets::BUILTIN_DEFAULT_ID)
                        .expect("invariant: builtin-default is in presets")
                }),
        }
    };

    // Merge vocabulary: global ∪ profile (profile wins on `wrong` collision).
    let merged_entries = {
        let global_entries = lock_or_recover(vocabulary).entries.clone();
        union_vocab(
            global_entries,
            profile.as_ref().map(|p| &p.vocabulary[..]).unwrap_or(&[]),
        )
    };

    let initial_prompt = build_initial_prompt(&merged_entries);
    let vocab_replacements: Vec<(String, String)> = merged_entries
        .iter()
        .map(|e| (e.wrong.clone(), e.correct.clone()))
        .collect();

    let compiled_transforms = custom_rules::compile_all(&style.custom_rules)?;

    let remove_fillers = match style.filler_override {
        Some(b) => b,
        None => remove_fillers_global,
    };

    let vocab_learning_enabled = profile
        .as_ref()
        .and_then(|p| p.vocabulary_learning_override)
        .unwrap_or(vocab_learning_global);

    let learning_target_profile_id = profile.as_ref().map(|p| p.id.clone());
    let matched_profile_id = profile.as_ref().map(|p| p.id.clone());
    let matched_style_id = Some(style.id.clone());

    Ok(EffectiveResolution {
        initial_prompt,
        vocab_replacements,
        formatting: style.formatting,
        compiled_transforms,
        correction: style.correction,
        remove_fillers,
        vocab_learning_enabled,
        learning_target_profile_id,
        matched_profile_id,
        matched_style_id,
    })
}

/// Union of global + profile vocabulary, dedup'd by lowercase `wrong`.
/// Profile entries win on collision.
fn union_vocab(global: Vec<VocabularyEntry>, profile: &[VocabularyEntry]) -> Vec<VocabularyEntry> {
    use std::collections::HashMap;
    let mut by_key: HashMap<String, VocabularyEntry> = HashMap::new();
    for e in global {
        by_key.insert(e.wrong.to_lowercase(), e);
    }
    for e in profile {
        by_key.insert(e.wrong.to_lowercase(), e.clone());
    }
    let mut out: Vec<VocabularyEntry> = by_key.into_values().collect();
    out.sort_by(|a, b| b.last_used.cmp(&a.last_used));
    out
}

/// Build the comma-separated initial-prompt string, capped at the budget.
fn build_initial_prompt(entries: &[VocabularyEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut prompt = String::new();
    for entry in entries {
        let word = &entry.correct;
        let addition = if prompt.is_empty() {
            word.clone()
        } else {
            format!(", {}", word)
        };
        if prompt.len() + addition.len() > INITIAL_PROMPT_CHAR_BUDGET {
            break;
        }
        prompt.push_str(&addition);
    }
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::{CasingMode, PunctuationMode};
    use crate::vocabulary::{Vocabulary, VocabularyEntry, VocabularySource};
    use parking_lot::Mutex;

    fn entry(wrong: &str, correct: &str, last_used: &str) -> VocabularyEntry {
        VocabularyEntry {
            wrong: wrong.into(),
            correct: correct.into(),
            source: VocabularySource::Manual,
            confidence: 1,
            created_at: last_used.into(),
            last_used: last_used.into(),
        }
    }

    #[test]
    fn no_profile_uses_default_style_and_global_vocab() {
        let current_app: Mutex<Option<FrontmostApp>> = Mutex::new(None);
        let profiles = Mutex::new(ProfilesStore::seeded());
        let styles = Mutex::new(StylesStore::seeded());
        let mut vocab = Vocabulary::default();
        vocab
            .entries
            .push(entry("Marshal", "Marcel", "2026-01-01T00:00:00Z"));
        let vocabulary = Mutex::new(vocab);
        let settings = Mutex::new(UserSettings::default());

        let r = resolve(&current_app, &profiles, &styles, &vocabulary, &settings).unwrap();
        assert!(r.initial_prompt.contains("Marcel"));
        assert_eq!(r.vocab_replacements.len(), 1);
        assert_eq!(r.formatting.casing, CasingMode::Sentence);
        assert_eq!(r.matched_profile_id, None);
        assert_eq!(
            r.matched_style_id.as_deref(),
            Some(style_presets::BUILTIN_DEFAULT_ID)
        );
    }

    #[test]
    fn matching_profile_uses_its_style() {
        let app = FrontmostApp {
            bundle_id: "com.apple.Terminal".into(),
            name: "Terminal".into(),
        };
        let current_app = Mutex::new(Some(app));
        let profiles = Mutex::new(ProfilesStore::seeded());
        let styles = Mutex::new(StylesStore::seeded());
        let vocabulary = Mutex::new(Vocabulary::default());
        let settings = Mutex::new(UserSettings::default());

        let r = resolve(&current_app, &profiles, &styles, &vocabulary, &settings).unwrap();
        assert_eq!(r.formatting.casing, CasingMode::Lowercase);
        assert!(matches!(r.formatting.punctuation, PunctuationMode::Strip));
        assert_eq!(
            r.matched_style_id.as_deref(),
            Some(style_presets::BUILTIN_PLAIN_LOWER_ID)
        );
    }

    #[test]
    fn disabled_profile_skipped() {
        let app = FrontmostApp {
            bundle_id: "com.apple.Terminal".into(),
            name: "Terminal".into(),
        };
        let current_app = Mutex::new(Some(app));
        let mut profiles_store = ProfilesStore::seeded();
        let term_id = profiles_store
            .profiles
            .iter()
            .find(|p| p.bundle_id == "com.apple.Terminal")
            .unwrap()
            .id
            .clone();
        profiles_store.set_enabled(&term_id, false).unwrap();
        let profiles = Mutex::new(profiles_store);
        let styles = Mutex::new(StylesStore::seeded());
        let vocabulary = Mutex::new(Vocabulary::default());
        let settings = Mutex::new(UserSettings::default());

        let r = resolve(&current_app, &profiles, &styles, &vocabulary, &settings).unwrap();
        assert_eq!(r.matched_profile_id, None);
        assert_eq!(r.formatting.casing, CasingMode::Sentence);
    }

    #[test]
    fn profile_vocab_wins_on_collision() {
        let app = FrontmostApp {
            bundle_id: "com.tinyspeck.slackmacgap".into(),
            name: "Slack".into(),
        };
        let current_app = Mutex::new(Some(app));

        let mut profiles_store = ProfilesStore::seeded();
        let slack = profiles_store
            .profiles
            .iter_mut()
            .find(|p| p.bundle_id == "com.tinyspeck.slackmacgap")
            .unwrap();
        slack
            .vocabulary
            .push(entry("api", "API", "2026-02-01T00:00:00Z"));
        let profiles = Mutex::new(profiles_store);

        let styles = Mutex::new(StylesStore::seeded());

        let mut vocab = Vocabulary::default();
        vocab
            .entries
            .push(entry("api", "Api", "2026-01-01T00:00:00Z"));
        let vocabulary = Mutex::new(vocab);
        let settings = Mutex::new(UserSettings::default());

        let r = resolve(&current_app, &profiles, &styles, &vocabulary, &settings).unwrap();
        // Profile vocab should win: replacement is "API", not "Api".
        let pair = r
            .vocab_replacements
            .iter()
            .find(|(w, _)| w.eq_ignore_ascii_case("api"))
            .unwrap();
        assert_eq!(pair.1, "API");
    }

    #[test]
    fn orphan_style_falls_back_to_default() {
        let app = FrontmostApp {
            bundle_id: "com.example.unknown".into(),
            name: "Unknown".into(),
        };
        let current_app = Mutex::new(Some(app.clone()));

        let mut profiles_store = ProfilesStore::default();
        profiles_store.profiles.push(crate::profiles::AppProfile {
            id: "p1".into(),
            bundle_id: "com.example.unknown".into(),
            display_name: "Unknown".into(),
            enabled: true,
            style_id: "nonexistent-style".into(),
            vocabulary: vec![],
            vocabulary_learning_override: None,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        });
        let profiles = Mutex::new(profiles_store);
        let styles = Mutex::new(StylesStore::seeded());
        let vocabulary = Mutex::new(Vocabulary::default());
        let settings = Mutex::new(UserSettings::default());

        let r = resolve(&current_app, &profiles, &styles, &vocabulary, &settings).unwrap();
        assert_eq!(
            r.matched_style_id.as_deref(),
            Some(style_presets::BUILTIN_DEFAULT_ID)
        );
    }
}
