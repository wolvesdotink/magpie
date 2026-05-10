use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// Activation mode for triggering dictation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ActivationMode {
    /// Hold Fn key to record, release to transcribe
    HoldFn,
    /// Double-tap Fn key to toggle recording
    DoubleTapFn,
    /// Use a keyboard shortcut (Cmd+Shift+Space)
    Shortcut,
}

impl Default for ActivationMode {
    fn default() -> Self {
        Self::HoldFn
    }
}

/// Persisted user settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
        }
    }
}

/// Get the path to the settings JSON file
fn settings_path() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "magpie", "Magpie")
        .context("Failed to determine app data directory")?;

    let data_dir = proj_dirs.data_dir();
    std::fs::create_dir_all(data_dir)
        .context("Failed to create data directory")?;

    Ok(data_dir.join("settings.json"))
}

impl UserSettings {
    /// Load settings from disk, falling back to defaults if the file
    /// is missing or corrupt.
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

        match std::fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(settings) => {
                    log::info!("Loaded settings from {}", path.display());
                    settings
                }
                Err(e) => {
                    log::warn!("Settings file is corrupt, using defaults: {}", e);
                    Self::default()
                }
            },
            Err(e) => {
                log::warn!("Failed to read settings file, using defaults: {}", e);
                Self::default()
            }
        }
    }

    /// Persist current settings to disk.
    pub fn save(&self) -> Result<()> {
        let path = settings_path()?;
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize settings")?;
        std::fs::write(&path, json)
            .context("Failed to write settings file")?;
        log::info!("Settings saved to {}", path.display());
        Ok(())
    }
}
