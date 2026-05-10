use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// How a vocabulary entry was created
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum VocabularySource {
    /// Automatically detected via correction monitoring
    Auto,
    /// Manually added by the user
    Manual,
}

/// A single vocabulary correction entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabularyEntry {
    /// What Whisper incorrectly produces
    pub wrong: String,
    /// What the user wants instead
    pub correct: String,
    /// How this entry was created
    pub source: VocabularySource,
    /// Incremented each time this correction is re-detected
    pub confidence: u32,
    /// When this entry was first created (ISO 8601)
    pub created_at: String,
    /// When this entry was last used/detected (ISO 8601)
    pub last_used: String,
}

/// The full vocabulary store
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vocabulary {
    pub version: u32,
    pub entries: Vec<VocabularyEntry>,
}

/// Maximum character budget for the Whisper initial_prompt.
/// Whisper's token limit is ~224 tokens; at ~4 chars/token this gives ~800 chars.
const INITIAL_PROMPT_CHAR_BUDGET: usize = 800;

/// Get the path to the vocabulary JSON file
fn vocabulary_path() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "magpie", "Magpie")
        .context("Failed to determine app data directory")?;

    let data_dir = proj_dirs.data_dir();
    std::fs::create_dir_all(data_dir).context("Failed to create data directory")?;

    Ok(data_dir.join("vocabulary.json"))
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

impl Vocabulary {
    /// Load vocabulary from disk, falling back to empty if the file
    /// is missing or corrupt.
    pub fn load() -> Self {
        let path = match vocabulary_path() {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Could not determine vocabulary path, using empty: {}", e);
                return Self::default();
            }
        };

        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(vocab) => {
                    log::info!("Loaded vocabulary from {}", path.display());
                    vocab
                }
                Err(e) => {
                    log::warn!("Vocabulary file is corrupt, using empty: {}", e);
                    Self::default()
                }
            },
            Err(e) => {
                log::warn!("Failed to read vocabulary file, using empty: {}", e);
                Self::default()
            }
        }
    }

    /// Persist current vocabulary to disk.
    pub fn save(&self) -> Result<()> {
        let path = vocabulary_path()?;
        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize vocabulary")?;
        std::fs::write(&path, json).context("Failed to write vocabulary file")?;
        log::info!(
            "Vocabulary saved to {} ({} entries)",
            path.display(),
            self.entries.len()
        );
        Ok(())
    }

    /// Add a new entry or update an existing one for the same `wrong` word.
    /// If the entry already exists, updates the `correct` value and increments confidence.
    pub fn add_or_update(&mut self, wrong: &str, correct: &str, source: VocabularySource) {
        let now = Utc::now().to_rfc3339();
        let wrong_lower = wrong.to_lowercase();

        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.wrong.to_lowercase() == wrong_lower)
        {
            entry.correct = correct.to_string();
            entry.confidence += 1;
            entry.last_used = now;
            log::info!(
                "Updated vocabulary entry: \"{}\" -> \"{}\" (confidence: {})",
                entry.wrong,
                entry.correct,
                entry.confidence
            );
        } else {
            self.entries.push(VocabularyEntry {
                wrong: wrong.to_string(),
                correct: correct.to_string(),
                source,
                confidence: 1,
                created_at: now.clone(),
                last_used: now,
            });
            log::info!(
                "Added vocabulary entry: \"{}\" -> \"{}\"",
                wrong,
                correct
            );
        }
    }

    /// Remove an entry by its `wrong` word (case-insensitive).
    pub fn remove(&mut self, wrong: &str) -> bool {
        let wrong_lower = wrong.to_lowercase();
        let before = self.entries.len();
        self.entries
            .retain(|e| e.wrong.to_lowercase() != wrong_lower);
        let removed = self.entries.len() < before;
        if removed {
            log::info!("Removed vocabulary entry for \"{}\"", wrong);
        }
        removed
    }

    /// Build a comma-separated string of correct words for Whisper's initial_prompt.
    /// Sorted by most-recently-used first, truncated to fit the token budget.
    pub fn get_initial_prompt_words(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        // Sort by last_used descending (most recent first)
        let mut sorted: Vec<&VocabularyEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.last_used.cmp(&a.last_used));

        let mut prompt = String::new();
        for entry in sorted {
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

    /// Get all (wrong, correct) pairs for post-processing replacement.
    pub fn get_replacements(&self) -> Vec<(String, String)> {
        self.entries
            .iter()
            .map(|e| (e.wrong.clone(), e.correct.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_new_entry() {
        let mut vocab = Vocabulary::default();
        vocab.add_or_update("Marshal", "Marcel", VocabularySource::Manual);
        assert_eq!(vocab.entries.len(), 1);
        assert_eq!(vocab.entries[0].wrong, "Marshal");
        assert_eq!(vocab.entries[0].correct, "Marcel");
        assert_eq!(vocab.entries[0].confidence, 1);
    }

    #[test]
    fn test_update_existing_entry() {
        let mut vocab = Vocabulary::default();
        vocab.add_or_update("Marshal", "Marcel", VocabularySource::Auto);
        vocab.add_or_update("marshal", "Marcel", VocabularySource::Auto);
        assert_eq!(vocab.entries.len(), 1);
        assert_eq!(vocab.entries[0].confidence, 2);
    }

    #[test]
    fn test_remove_entry() {
        let mut vocab = Vocabulary::default();
        vocab.add_or_update("Marshal", "Marcel", VocabularySource::Manual);
        assert!(vocab.remove("marshal")); // case-insensitive
        assert_eq!(vocab.entries.len(), 0);
        assert!(!vocab.remove("nonexistent"));
    }

    #[test]
    fn test_initial_prompt_generation() {
        let mut vocab = Vocabulary::default();
        vocab.add_or_update("Marshal", "Marcel", VocabularySource::Manual);
        vocab.add_or_update("cubernetes", "Kubernetes", VocabularySource::Manual);
        let prompt = vocab.get_initial_prompt_words();
        // Both words should appear, comma-separated
        assert!(prompt.contains("Marcel"));
        assert!(prompt.contains("Kubernetes"));
        assert!(prompt.contains(", "));
    }

    #[test]
    fn test_initial_prompt_truncation() {
        let mut vocab = Vocabulary::default();
        // Add many long entries to exceed the budget
        for i in 0..200 {
            vocab.add_or_update(
                &format!("wrongword{}", i),
                &format!("superlongcorrectword{}", i),
                VocabularySource::Manual,
            );
        }
        let prompt = vocab.get_initial_prompt_words();
        assert!(prompt.len() <= INITIAL_PROMPT_CHAR_BUDGET);
    }

    #[test]
    fn test_get_replacements() {
        let mut vocab = Vocabulary::default();
        vocab.add_or_update("Marshal", "Marcel", VocabularySource::Manual);
        vocab.add_or_update("cubernetes", "Kubernetes", VocabularySource::Manual);
        let replacements = vocab.get_replacements();
        assert_eq!(replacements.len(), 2);
        assert!(replacements.contains(&("Marshal".to_string(), "Marcel".to_string())));
        assert!(replacements
            .contains(&("cubernetes".to_string(), "Kubernetes".to_string())));
    }
}
