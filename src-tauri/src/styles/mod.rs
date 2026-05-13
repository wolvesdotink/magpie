//! Reusable styling artifacts ("Styles") that can be applied to one or more
//! per-app profiles. A Style bundles formatting rules, an optional correction
//! prompt override, and an ordered pipeline of user-defined text transforms.
//!
//! Stored at `~/Library/Application Support/Magpie/styles.json` using the
//! same versioned-envelope pattern as `settings.json`.

pub mod error;
pub mod migrations;
pub mod presets;

pub use error::{Result, StylesError};

use std::path::PathBuf;

use chrono::Utc;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ──────────────────────────────────────────────────────────────────────────
// Casing / Punctuation enums
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CasingMode {
    /// Default — capitalize first letter (and optionally after sentences).
    #[default]
    Sentence,
    /// Pass through whisper's casing untouched.
    Preserve,
    Lowercase,
    Uppercase,
    SnakeCase,
    KebabCase,
    CamelCase,
    PascalCase,
    ScreamSnake,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PunctuationMode {
    /// Keep whisper's punctuation (current behavior).
    #[default]
    Auto,
    /// Strip all common punctuation.
    Strip,
    /// Keep only sentence-ending punctuation (. ! ?).
    SentenceOnly,
    /// User-defined allow-list of characters to KEEP.
    Custom { chars: Vec<char> },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CorrectionOverride {
    /// Use global self_correction toggle + default prompt.
    #[default]
    Inherit,
    /// Skip correction entirely.
    Disabled,
    /// Built-in casual cleanup prompt.
    Casual,
    /// Built-in formal cleanup prompt.
    Formal,
    /// User-defined system prompt (length-capped at 2048 chars).
    Custom { prompt: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattingRules {
    pub casing: CasingMode,
    pub punctuation: PunctuationMode,
    pub remove_trailing_period: bool,
    /// Only used when `casing == Sentence`: also capitalize after `. ! ?`.
    pub auto_capitalize_after_sentence: bool,
    /// Collapse runs of whitespace to a single space (default true).
    pub collapse_whitespace: bool,
}

impl Default for FormattingRules {
    fn default() -> Self {
        Self {
            casing: CasingMode::Sentence,
            punctuation: PunctuationMode::Auto,
            remove_trailing_period: false,
            auto_capitalize_after_sentence: false,
            collapse_whitespace: true,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Custom rule pipeline
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TransformKind {
    /// Literal or regex find/replace.
    Replace {
        pattern: String,
        replacement: String,
        #[serde(default)]
        is_regex: bool,
        #[serde(default)]
        case_sensitive: bool,
        #[serde(default)]
        whole_word: bool,
    },
    /// Prepend literal text to the entire transcription.
    Prepend { text: String },
    /// Append literal text to the entire transcription.
    Append { text: String },
    /// Strip leading and trailing whitespace.
    TrimEdges,
    /// Replace any run of matching chars with a single space.
    SqueezeChars { chars: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextTransform {
    pub id: String,
    pub enabled: bool,
    pub label: Option<String>,
    pub kind: TransformKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub ok: bool,
    pub error: Option<String>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
        }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Style itself
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub builtin: bool,
    pub formatting: FormattingRules,
    pub correction: CorrectionOverride,
    #[serde(default)]
    pub custom_rules: Vec<TextTransform>,
    #[serde(default)]
    pub filler_override: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
}

impl Style {
    /// Create a new user style with sensible defaults and a fresh UUID-like id.
    pub fn new_user(name: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: generate_id("style"),
            name: name.into(),
            description: None,
            builtin: false,
            formatting: FormattingRules::default(),
            correction: CorrectionOverride::Inherit,
            custom_rules: Vec::new(),
            filler_override: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Storage
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct StylesFile {
    version: u32,
    styles: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StylesStore {
    pub styles: Vec<Style>,
}

impl StylesStore {
    /// Load styles from disk, seeding built-in styles on first run.
    pub fn load() -> Self {
        let path = match styles_path() {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Could not determine styles path, using defaults: {}", e);
                return Self::seeded();
            }
        };

        if !path.exists() {
            let store = Self::seeded();
            if let Err(e) = store.save_to(&path) {
                log::warn!("Failed to seed styles.json: {}", e);
            }
            return store;
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to read styles file, using defaults: {}", e);
                return Self::seeded();
            }
        };

        match parse_versioned(&contents) {
            Ok(mut store) => {
                // Always ensure built-in styles are present so a hand-edited
                // file missing them doesn't leave dangling profile FKs.
                store.ensure_builtins();
                log::info!(
                    "Loaded {} styles from {}",
                    store.styles.len(),
                    path.display()
                );
                store
            }
            Err(StylesError::VersionTooNew { found, supported }) => {
                let backup = path.with_file_name("styles.future-backup.json");
                let _ = std::fs::rename(&path, &backup);
                log::error!(
                    "Styles file is from a newer version (v{found}, supported up to v{supported}). \
                     Backed up to {} and seeding defaults.",
                    backup.display()
                );
                Self::seeded()
            }
            Err(e) => {
                log::warn!("Styles file unusable ({}); seeding defaults", e);
                Self::seeded()
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = styles_path()?;
        self.save_to(&path)
    }

    fn save_to(&self, path: &std::path::Path) -> Result<()> {
        let json = serialize_versioned(self)?;
        std::fs::write(path, json).map_err(|source| StylesError::Io {
            path: path.to_path_buf(),
            source,
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
                log::warn!(
                    "Could not set styles file permissions to 0600 (data is written; \
                     readable by other local accounts): {e}"
                );
            }
        }
        log::info!(
            "Styles saved to {} ({} styles)",
            path.display(),
            self.styles.len()
        );
        Ok(())
    }

    pub fn seeded() -> Self {
        Self {
            styles: presets::builtin_styles(),
        }
    }

    /// Make sure every built-in id is present in the store. Missing ones are
    /// re-installed at the end of the list; existing built-ins (possibly
    /// user-edited) are left alone.
    pub fn ensure_builtins(&mut self) {
        let mut changed = false;
        for builtin in presets::builtin_styles() {
            if !self.styles.iter().any(|s| s.id == builtin.id) {
                self.styles.push(builtin);
                changed = true;
            }
        }
        if changed {
            log::info!("Re-seeded missing built-in styles");
        }
    }

    pub fn get(&self, id: &str) -> Option<&Style> {
        self.styles.iter().find(|s| s.id == id)
    }

    pub fn get_or_default(&self, id: &str) -> &Style {
        self.get(id)
            .or_else(|| self.get(presets::BUILTIN_DEFAULT_ID))
            .expect("invariant: builtin-default style is always seeded")
    }

    pub fn add(&mut self, mut style: Style) -> Style {
        if style.id.is_empty() {
            style.id = generate_id("style");
        }
        // user-created additions are never built-in regardless of payload
        style.builtin = false;
        let now = Utc::now().to_rfc3339();
        style.created_at = now.clone();
        style.updated_at = now;
        self.styles.push(style.clone());
        style
    }

    pub fn update(&mut self, id: &str, mut style: Style) -> Result<Style> {
        let existing = self
            .styles
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| StylesError::NotFound { id: id.to_string() })?;

        // Built-in flag can't be flipped via the update path; it stays as-is
        // and protects the delete path from being abused.
        style.builtin = existing.builtin;
        style.id = existing.id.clone();
        style.created_at = existing.created_at.clone();
        style.updated_at = Utc::now().to_rfc3339();

        *existing = style.clone();
        Ok(style)
    }

    pub fn delete(&mut self, id: &str) -> Result<()> {
        let pos = self
            .styles
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| StylesError::NotFound { id: id.to_string() })?;

        let style = &self.styles[pos];
        if style.builtin {
            return Err(StylesError::BuiltinDelete {
                name: style.name.clone(),
            });
        }
        self.styles.remove(pos);
        Ok(())
    }

    pub fn duplicate(&mut self, id: &str) -> Result<Style> {
        let src = self
            .styles
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| StylesError::NotFound { id: id.to_string() })?
            .clone();

        let now = Utc::now().to_rfc3339();
        let dup = Style {
            id: generate_id("style"),
            name: format!("{} (Copy)", src.name),
            description: src.description,
            builtin: false,
            formatting: src.formatting,
            correction: src.correction,
            custom_rules: src.custom_rules,
            filler_override: src.filler_override,
            created_at: now.clone(),
            updated_at: now,
        };
        self.styles.push(dup.clone());
        Ok(dup)
    }

    pub fn reset_to_default(&mut self, id: &str) -> Result<Style> {
        let defaults = presets::builtin_styles();
        let default_style = defaults
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| StylesError::NotFound { id: id.to_string() })?;

        let existing = self
            .styles
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| StylesError::NotFound { id: id.to_string() })?;
        existing.formatting = default_style.formatting;
        existing.correction = default_style.correction;
        existing.custom_rules = default_style.custom_rules;
        existing.filler_override = default_style.filler_override;
        existing.name = default_style.name;
        existing.description = default_style.description;
        existing.updated_at = Utc::now().to_rfc3339();
        Ok(existing.clone())
    }
}

fn styles_path() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "magpie", "Magpie").ok_or(StylesError::NoDataDir)?;
    let data_dir = proj_dirs.data_dir();
    std::fs::create_dir_all(data_dir).map_err(|source| StylesError::Io {
        path: data_dir.to_path_buf(),
        source,
    })?;
    Ok(data_dir.join("styles.json"))
}

pub fn parse_versioned(contents: &str) -> Result<StylesStore> {
    let raw: Value = serde_json::from_str(contents)?;
    let (mut payload, from_version) = match raw.get("version").and_then(Value::as_u64) {
        Some(v) => {
            let inner = raw
                .get("styles")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            (inner, v as u32)
        }
        None => (raw, 0),
    };

    migrations::run_migrations(from_version, &mut payload)?;

    let styles: Vec<Style> = serde_json::from_value(payload)?;
    Ok(StylesStore { styles })
}

pub fn serialize_versioned(store: &StylesStore) -> Result<String> {
    let file = StylesFile {
        version: migrations::CURRENT_VERSION,
        styles: serde_json::to_value(&store.styles)?,
    };
    Ok(serde_json::to_string_pretty(&file)?)
}

fn generate_id(prefix: &str) -> String {
    // Lightweight UUID-ish ID — no extra dep needed. Combines a millisecond
    // timestamp with a per-process counter so back-to-back creations stay
    // sortable and unique.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ts = chrono::Utc::now().timestamp_millis();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", prefix, ts, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_current_version() {
        let store = StylesStore::seeded();
        let json = serialize_versioned(&store).expect("serialize ok");
        let reloaded = parse_versioned(&json).expect("reload ok");
        assert_eq!(reloaded.styles.len(), store.styles.len());
    }

    #[test]
    fn rejects_future_version() {
        let future = format!(
            r#"{{"version": {}, "styles": []}}"#,
            migrations::CURRENT_VERSION + 1
        );
        let err = parse_versioned(&future).unwrap_err();
        assert!(matches!(err, StylesError::VersionTooNew { .. }));
    }

    #[test]
    fn seeded_contains_all_builtins() {
        let store = StylesStore::seeded();
        for id in presets::BUILTIN_IDS {
            assert!(
                store.styles.iter().any(|s| &s.id == id),
                "missing builtin id: {id}"
            );
        }
    }

    #[test]
    fn ensure_builtins_repairs_missing() {
        let mut store = StylesStore { styles: vec![] };
        store.ensure_builtins();
        for id in presets::BUILTIN_IDS {
            assert!(
                store.styles.iter().any(|s| &s.id == id),
                "missing builtin id: {id}"
            );
        }
    }

    #[test]
    fn delete_builtin_rejected() {
        let mut store = StylesStore::seeded();
        let err = store.delete(presets::BUILTIN_DEFAULT_ID).unwrap_err();
        assert!(matches!(err, StylesError::BuiltinDelete { .. }));
    }

    #[test]
    fn delete_user_style_works() {
        let mut store = StylesStore::seeded();
        let added = store.add(Style::new_user("Throwaway"));
        store.delete(&added.id).expect("delete user style");
        assert!(store.get(&added.id).is_none());
    }

    #[test]
    fn add_user_clears_builtin_flag_even_if_set() {
        let mut store = StylesStore::seeded();
        let mut payload = Style::new_user("Sneaky");
        payload.builtin = true;
        let added = store.add(payload);
        assert!(!added.builtin);
    }

    #[test]
    fn update_preserves_builtin_flag() {
        let mut store = StylesStore::seeded();
        let mut style = store.get(presets::BUILTIN_CASUAL_ID).unwrap().clone();
        style.builtin = false; // try to flip
        style.name = "Casual Custom".into();
        let updated = store.update(presets::BUILTIN_CASUAL_ID, style).unwrap();
        assert!(updated.builtin, "builtin flag must be preserved");
    }

    #[test]
    fn duplicate_creates_independent_user_style() {
        let mut store = StylesStore::seeded();
        let dup = store.duplicate(presets::BUILTIN_CASUAL_ID).unwrap();
        assert!(!dup.builtin);
        assert_ne!(dup.id, presets::BUILTIN_CASUAL_ID);
        assert!(dup.name.contains("(Copy)"));
    }
}
