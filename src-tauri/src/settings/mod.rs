pub mod error;
pub mod migrations;

pub use error::SettingsError;

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_true() -> bool {
    true
}

/// Activation mode for triggering dictation
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ActivationMode {
    /// Hold Fn key to record, release to transcribe
    #[default]
    HoldFn,
    /// Single-tap Fn key to toggle recording (fires on every Fn press,
    /// including macOS Fn-modifier shortcuts like Fn+F1)
    TapFn,
    /// Double-tap Fn key to toggle recording
    DoubleTapFn,
    /// Use a global keyboard shortcut (user-configurable; defaults to
    /// Cmd+Shift+Space)
    Shortcut,
}

/// Persisted user settings
///
/// `#[serde(default)]` on the struct means a settings JSON missing any field
/// gets that field filled in from `UserSettings::default()`. This is what
/// makes a future migration safe to add a brand-new field without manually
/// patching every pre-existing settings.json on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserSettings {
    /// Which activation mode to use
    pub activation_mode: ActivationMode,
    /// Language for transcription (None = auto-detect)
    pub language: Option<String>,
    /// Selected model identifier (e.g. "base.en")
    pub selected_model: Option<String>,
    /// Whether to auto-start on login
    pub auto_start: bool,
    /// List of filler words to remove
    pub filler_words: Vec<String>,
    /// Whether filler word removal is enabled
    pub remove_fillers: bool,
    /// Whether self-correction detection is enabled
    #[serde(default)]
    pub self_correction: bool,
    /// Selected correction model identifier (e.g. "qwen2.5-0.5b")
    #[serde(default)]
    pub selected_correction_model: Option<String>,
    /// Whether automatic vocabulary learning from corrections is enabled
    #[serde(default = "default_true")]
    pub vocabulary_learning: bool,
    /// Whether the first-launch setup wizard has been completed
    #[serde(default)]
    pub setup_complete: bool,
    /// Whether the streaming-preview worker emits live partial captions
    /// while recording. When false, only the final transcript on stop runs.
    /// Defaults to false (simpler, lower CPU, fewer moving parts). Existing
    /// users without this field in `settings.json` also default to false.
    #[serde(default)]
    pub streaming_preview: bool,
    /// User-customizable global shortcut string in Tauri's format
    /// (e.g. "CmdOrCtrl+Shift+Space"). `None` falls back to the built-in
    /// default. Existing settings.json files without this field load as None.
    #[serde(default)]
    pub custom_shortcut: Option<String>,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            activation_mode: ActivationMode::HoldFn,
            language: None,
            selected_model: None,
            auto_start: false,
            filler_words: vec![
                "um".to_string(),
                "uh".to_string(),
                "hmm".to_string(),
                "mm".to_string(),
                "ah".to_string(),
                "er".to_string(),
            ],
            remove_fillers: true,
            self_correction: false,
            selected_correction_model: None,
            vocabulary_learning: true,
            setup_complete: false,
            streaming_preview: false,
            custom_shortcut: None,
        }
    }
}

/// Versioned envelope written to disk since Phase 1. Older v0 files are
/// bare `UserSettings` documents without this wrapper; the loader detects
/// that shape by the absence of a top-level `version` field and treats it
/// as v0 before migrating.
#[derive(Debug, Serialize, Deserialize)]
struct SettingsFile {
    version: u32,
    settings: Value,
}

/// Get the path to the settings JSON file
fn settings_path() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "magpie", "Magpie")
        .context("Failed to determine app data directory")?;

    let data_dir = proj_dirs.data_dir();
    std::fs::create_dir_all(data_dir).context("Failed to create data directory")?;

    Ok(data_dir.join("settings.json"))
}

/// Parse settings from a string (the on-disk JSON contents), running any
/// required migrations forward to [`migrations::CURRENT_VERSION`].
///
/// Pure function — no filesystem access — so it is the unit-test entry
/// point for migration coverage. The public [`UserSettings::load`] wraps
/// this with disk I/O and the default-on-error fallback.
pub fn parse_versioned_settings(
    contents: &str,
) -> std::result::Result<UserSettings, SettingsError> {
    let raw: Value = serde_json::from_str(contents)?;

    // Detect v0 (bare UserSettings) vs. versioned envelope.
    let (mut payload, from_version) = match raw.get("version").and_then(Value::as_u64) {
        Some(v) => {
            let inner = raw
                .get("settings")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            (inner, v as u32)
        }
        None => (raw, 0),
    };

    migrations::run_migrations(from_version, &mut payload)?;

    let settings: UserSettings = serde_json::from_value(payload)?;
    Ok(settings)
}

/// Serialize settings into the current versioned envelope. Pure function;
/// `UserSettings::save` wraps this with disk I/O.
pub fn serialize_versioned_settings(
    settings: &UserSettings,
) -> std::result::Result<String, SettingsError> {
    let file = SettingsFile {
        version: migrations::CURRENT_VERSION,
        settings: serde_json::to_value(settings)?,
    };
    Ok(serde_json::to_string_pretty(&file)?)
}

impl UserSettings {
    /// Load settings from disk, migrating older versions forward and falling
    /// back to defaults if the file is missing or unreadable.
    ///
    /// A *corrupt* file (parse failure or migration error) is logged and
    /// replaced with defaults rather than propagated, so a bad upgrade
    /// can't permanently lock the user out of the app.
    ///
    /// A *future-version* file (older build trying to read newer settings)
    /// is renamed to `settings.future-backup.json` BEFORE we fall back to
    /// defaults — otherwise the next `save()` would overwrite the user's
    /// future-version data with v1 defaults. The backup is non-destructive
    /// so a downgrade-then-upgrade cycle preserves the original state.
    pub fn load() -> Self {
        let path = match settings_path() {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Could not determine settings path, using defaults: {}", e);
                return Self::default();
            }
        };

        if !path.exists() {
            return Self::default();
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to read settings file, using defaults: {}", e);
                return Self::default();
            }
        };

        match parse_versioned_settings(&contents) {
            Ok(settings) => {
                log::info!("Loaded settings from {}", path.display());
                settings
            }
            Err(SettingsError::VersionTooNew { found, supported }) => {
                let backup = path.with_file_name("settings.future-backup.json");
                match std::fs::rename(&path, &backup) {
                    Ok(()) => log::error!(
                        "Settings file is from a newer version (v{found}, this build supports up to v{supported}). \
                         Backed up to {} and falling back to defaults so we don't overwrite your data.",
                        backup.display()
                    ),
                    Err(io_err) => log::error!(
                        "Settings file is from a newer version (v{found}, this build supports up to v{supported}). \
                         Failed to back up to {}: {io_err}. Falling back to defaults; the original file will be \
                         overwritten on the next save unless you move it manually.",
                        backup.display()
                    ),
                }
                Self::default()
            }
            Err(e) => {
                log::warn!("Settings file unusable ({}); using defaults", e);
                Self::default()
            }
        }
    }

    /// Persist current settings to disk as the versioned envelope.
    pub fn save(&self) -> Result<()> {
        let path = settings_path()?;
        let json = serialize_versioned_settings(self).context("Failed to serialize settings")?;
        std::fs::write(&path, json).context("Failed to write settings file")?;
        log::info!("Settings saved to {}", path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pre-Phase-1 (v0) settings file: bare `UserSettings`, no envelope.
    const V0_FIXTURE: &str = r#"{
        "activationMode": "holdFn",
        "language": null,
        "selectedModel": "small.en",
        "autoStart": false,
        "fillerWords": ["um", "uh"],
        "removeFillers": true
    }"#;

    #[test]
    fn parses_v0_bare_shape_as_v_current() {
        let settings = parse_versioned_settings(V0_FIXTURE).expect("v0 fixture loads");
        assert_eq!(settings.selected_model.as_deref(), Some("small.en"));
        assert!(settings.remove_fillers);
        // Field added after v0 with #[serde(default)] should take its default.
        assert!(
            settings.vocabulary_learning,
            "vocabulary_learning default = true"
        );
        assert!(!settings.setup_complete);
    }

    #[test]
    fn round_trips_current_version() {
        let original = UserSettings {
            selected_model: Some("base.en".into()),
            auto_start: true,
            ..UserSettings::default()
        };
        let json = serialize_versioned_settings(&original).expect("serialize ok");
        let reloaded = parse_versioned_settings(&json).expect("reload ok");
        assert_eq!(reloaded.selected_model, original.selected_model);
        assert_eq!(reloaded.auto_start, original.auto_start);
    }

    #[test]
    fn serialized_payload_uses_versioned_envelope() {
        let json = serialize_versioned_settings(&UserSettings::default()).expect("serialize ok");
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.get("version").and_then(Value::as_u64),
            Some(migrations::CURRENT_VERSION as u64),
            "envelope must include current version"
        );
        assert!(
            parsed.get("settings").is_some(),
            "envelope must wrap payload"
        );
    }

    #[test]
    fn rejects_future_version() {
        let future = format!(
            r#"{{"version": {}, "settings": {{}}}}"#,
            migrations::CURRENT_VERSION + 1
        );
        let err = parse_versioned_settings(&future).unwrap_err();
        assert!(
            matches!(err, SettingsError::VersionTooNew { .. }),
            "expected VersionTooNew, got {err:?}"
        );
    }

    #[test]
    fn corrupt_json_returns_parse_error() {
        let err = parse_versioned_settings("not json {{{").unwrap_err();
        assert!(matches!(err, SettingsError::Parse(_)));
    }

    #[test]
    fn empty_envelope_payload_uses_defaults() {
        let json = format!(
            r#"{{"version": {}, "settings": {{}}}}"#,
            migrations::CURRENT_VERSION
        );
        let settings = parse_versioned_settings(&json).expect("empty payload OK");
        // Every field should match Default::default() because the empty
        // object lets serde fill in #[serde(default)]s.
        let defaults = UserSettings::default();
        assert_eq!(settings.activation_mode, defaults.activation_mode);
        assert_eq!(settings.selected_model, defaults.selected_model);
        assert_eq!(settings.vocabulary_learning, defaults.vocabulary_learning);
    }
}
