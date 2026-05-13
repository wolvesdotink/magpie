//! Per-app profiles. Each profile binds a bundle ID to a Style (by ID),
//! plus a profile-local vocabulary list and an auto-learning override.
//!
//! Stored at `~/Library/Application Support/Magpie/profiles.json`.

pub mod error;
pub mod migrations;
pub mod presets;

pub use error::{ProfilesError, Result};

use std::path::PathBuf;

use chrono::Utc;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::vocabulary::VocabularyEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppProfile {
    pub id: String,
    pub bundle_id: String,
    pub display_name: String,
    pub enabled: bool,
    /// FK to Style.id
    pub style_id: String,
    #[serde(default)]
    pub vocabulary: Vec<VocabularyEntry>,
    /// None = inherit global vocabulary_learning, Some(true/false) = override.
    #[serde(default)]
    pub vocabulary_learning_override: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
}

impl AppProfile {
    pub fn new_user(
        bundle_id: impl Into<String>,
        display_name: impl Into<String>,
        style_id: impl Into<String>,
    ) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: generate_id("profile"),
            bundle_id: bundle_id.into(),
            display_name: display_name.into(),
            enabled: true,
            style_id: style_id.into(),
            vocabulary: vec![],
            vocabulary_learning_override: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ProfilesFile {
    version: u32,
    profiles: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfilesStore {
    pub profiles: Vec<AppProfile>,
}

impl ProfilesStore {
    pub fn load() -> Self {
        let path = match profiles_path() {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Could not determine profiles path, using defaults: {}", e);
                return Self::seeded();
            }
        };

        if !path.exists() {
            let store = Self::seeded();
            if let Err(e) = store.save_to(&path) {
                log::warn!("Failed to seed profiles.json: {}", e);
            }
            return store;
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to read profiles file, using defaults: {}", e);
                return Self::seeded();
            }
        };

        match parse_versioned(&contents) {
            Ok(store) => {
                log::info!(
                    "Loaded {} profiles from {}",
                    store.profiles.len(),
                    path.display()
                );
                store
            }
            Err(ProfilesError::VersionTooNew { found, supported }) => {
                let backup = path.with_file_name("profiles.future-backup.json");
                let _ = std::fs::rename(&path, &backup);
                log::error!(
                    "Profiles file is from a newer version (v{found}, supported up to v{supported}). \
                     Backed up to {} and seeding defaults.",
                    backup.display()
                );
                Self::seeded()
            }
            Err(e) => {
                log::warn!("Profiles file unusable ({}); seeding defaults", e);
                Self::seeded()
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = profiles_path()?;
        self.save_to(&path)
    }

    fn save_to(&self, path: &std::path::Path) -> Result<()> {
        let json = serialize_versioned(self)?;
        std::fs::write(path, json).map_err(|source| ProfilesError::Io {
            path: path.to_path_buf(),
            source,
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
                log::warn!(
                    "Could not set profiles file permissions to 0600 (data is written; \
                     readable by other local accounts): {e}"
                );
            }
        }
        log::info!(
            "Profiles saved to {} ({} profiles)",
            path.display(),
            self.profiles.len()
        );
        Ok(())
    }

    pub fn seeded() -> Self {
        Self {
            profiles: presets::builtin_profiles(),
        }
    }

    /// Make sure every built-in bundle_id has a profile. Existing profiles
    /// are left alone (the user may have edited or even reassigned styles).
    pub fn ensure_builtins(&mut self) {
        let mut changed = false;
        for builtin in presets::builtin_profiles() {
            if !self
                .profiles
                .iter()
                .any(|p| p.bundle_id == builtin.bundle_id)
            {
                self.profiles.push(builtin);
                changed = true;
            }
        }
        if changed {
            log::info!("Re-seeded missing built-in profiles");
        }
    }

    pub fn find_by_bundle(&self, bundle_id: &str) -> Option<&AppProfile> {
        self.profiles
            .iter()
            .find(|p| p.enabled && p.bundle_id == bundle_id)
    }

    pub fn get(&self, id: &str) -> Option<&AppProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn add(&mut self, mut profile: AppProfile) -> AppProfile {
        if profile.id.is_empty() {
            profile.id = generate_id("profile");
        }
        let now = Utc::now().to_rfc3339();
        profile.created_at = now.clone();
        profile.updated_at = now;
        self.profiles.push(profile.clone());
        profile
    }

    pub fn update(&mut self, id: &str, mut profile: AppProfile) -> Result<AppProfile> {
        let existing = self
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| ProfilesError::NotFound { id: id.to_string() })?;
        profile.id = existing.id.clone();
        profile.created_at = existing.created_at.clone();
        profile.updated_at = Utc::now().to_rfc3339();
        *existing = profile.clone();
        Ok(profile)
    }

    pub fn delete(&mut self, id: &str) -> Result<()> {
        let pos = self
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| ProfilesError::NotFound { id: id.to_string() })?;
        self.profiles.remove(pos);
        Ok(())
    }

    pub fn duplicate(&mut self, id: &str) -> Result<AppProfile> {
        let src = self
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| ProfilesError::NotFound { id: id.to_string() })?
            .clone();
        let now = Utc::now().to_rfc3339();
        let dup = AppProfile {
            id: generate_id("profile"),
            bundle_id: src.bundle_id,
            display_name: format!("{} (Copy)", src.display_name),
            enabled: false, // disable copies by default so they don't fight for the bundle
            style_id: src.style_id,
            vocabulary: src.vocabulary,
            vocabulary_learning_override: src.vocabulary_learning_override,
            created_at: now.clone(),
            updated_at: now,
        };
        self.profiles.push(dup.clone());
        Ok(dup)
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<()> {
        let existing = self
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| ProfilesError::NotFound { id: id.to_string() })?;
        existing.enabled = enabled;
        existing.updated_at = Utc::now().to_rfc3339();
        Ok(())
    }

    /// Add or update a vocabulary entry on a specific profile.
    pub fn add_vocab_to_profile(
        &mut self,
        profile_id: &str,
        wrong: &str,
        correct: &str,
        source: crate::vocabulary::VocabularySource,
    ) -> Result<()> {
        let profile = self
            .profiles
            .iter_mut()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| ProfilesError::NotFound {
                id: profile_id.to_string(),
            })?;
        upsert_vocab_entry(&mut profile.vocabulary, wrong, correct, source);
        profile.updated_at = Utc::now().to_rfc3339();
        Ok(())
    }
}

fn upsert_vocab_entry(
    entries: &mut Vec<VocabularyEntry>,
    wrong: &str,
    correct: &str,
    source: crate::vocabulary::VocabularySource,
) {
    let now = Utc::now().to_rfc3339();
    let wrong_lower = wrong.to_lowercase();
    if let Some(entry) = entries
        .iter_mut()
        .find(|e| e.wrong.to_lowercase() == wrong_lower)
    {
        entry.correct = correct.to_string();
        entry.confidence += 1;
        entry.last_used = now;
    } else {
        entries.push(VocabularyEntry {
            wrong: wrong.to_string(),
            correct: correct.to_string(),
            source,
            confidence: 1,
            created_at: now.clone(),
            last_used: now,
        });
    }
}

fn profiles_path() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "magpie", "Magpie").ok_or(ProfilesError::NoDataDir)?;
    let data_dir = proj_dirs.data_dir();
    std::fs::create_dir_all(data_dir).map_err(|source| ProfilesError::Io {
        path: data_dir.to_path_buf(),
        source,
    })?;
    Ok(data_dir.join("profiles.json"))
}

pub fn parse_versioned(contents: &str) -> Result<ProfilesStore> {
    let raw: Value = serde_json::from_str(contents)?;
    let (mut payload, from_version) = match raw.get("version").and_then(Value::as_u64) {
        Some(v) => {
            let inner = raw
                .get("profiles")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            (inner, v as u32)
        }
        None => (raw, 0),
    };
    migrations::run_migrations(from_version, &mut payload)?;
    let profiles: Vec<AppProfile> = serde_json::from_value(payload)?;
    Ok(ProfilesStore { profiles })
}

pub fn serialize_versioned(store: &ProfilesStore) -> Result<String> {
    let file = ProfilesFile {
        version: migrations::CURRENT_VERSION,
        profiles: serde_json::to_value(&store.profiles)?,
    };
    Ok(serde_json::to_string_pretty(&file)?)
}

fn generate_id(prefix: &str) -> String {
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
        let store = ProfilesStore::seeded();
        let json = serialize_versioned(&store).expect("serialize ok");
        let reloaded = parse_versioned(&json).expect("reload ok");
        assert_eq!(reloaded.profiles.len(), store.profiles.len());
    }

    #[test]
    fn rejects_future_version() {
        let future = format!(
            r#"{{"version": {}, "profiles": []}}"#,
            migrations::CURRENT_VERSION + 1
        );
        let err = parse_versioned(&future).unwrap_err();
        assert!(matches!(err, ProfilesError::VersionTooNew { .. }));
    }

    #[test]
    fn seeded_contains_known_apps() {
        let store = ProfilesStore::seeded();
        let bundles: Vec<&str> = store.profiles.iter().map(|p| p.bundle_id.as_str()).collect();
        assert!(bundles.contains(&"com.tinyspeck.slackmacgap"));
        assert!(bundles.contains(&"com.apple.mail"));
        assert!(bundles.contains(&"com.apple.Terminal"));
    }

    #[test]
    fn find_by_bundle_skips_disabled() {
        let mut store = ProfilesStore::seeded();
        let id = store.profiles[0].id.clone();
        let bundle = store.profiles[0].bundle_id.clone();
        store.set_enabled(&id, false).unwrap();
        assert!(store.find_by_bundle(&bundle).is_none());
    }

    #[test]
    fn ensure_builtins_repairs_missing() {
        let mut store = ProfilesStore { profiles: vec![] };
        store.ensure_builtins();
        for preset in presets::builtin_profiles() {
            assert!(
                store
                    .profiles
                    .iter()
                    .any(|p| p.bundle_id == preset.bundle_id),
                "missing builtin bundle: {}",
                preset.bundle_id
            );
        }
    }

    #[test]
    fn duplicate_is_disabled_by_default() {
        let mut store = ProfilesStore::seeded();
        let id = store.profiles[0].id.clone();
        let dup = store.duplicate(&id).unwrap();
        assert!(!dup.enabled);
        assert!(dup.display_name.contains("(Copy)"));
    }
}
