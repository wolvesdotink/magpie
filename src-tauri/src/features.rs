//! Feature flags.
//!
//! Centralizes "is feature X enabled?" decisions in one place so callers
//! query the resolved state rather than reading settings + env + build cfg
//! ad hoc. Decisions resolve in this order, highest precedence first:
//!
//! 1. **Environment override** — `MAGPIE_FEATURE_<flag_in_screaming_snake>=1`
//!    or `=0` forces the value. Useful during dev (`MAGPIE_FEATURE_FILE_IMPORT=1
//!    bun tauri dev`) and in CI smoke tests. Never read from in shipped builds
//!    by accident — the env var has to be explicitly set.
//! 2. **User settings** — values stored in `UserSettings` (today only
//!    `streaming_preview`). Future flags can opt in to user-facing toggles
//!    by adding a corresponding field.
//! 3. **Compile-time defaults** — [`FeatureFlags::default`] is the safe
//!    fallback for a fresh install or a flag the user has not touched.
//!
//! Lifecycle, per ADR-0002:
//!   1. Flag added, defaults `false`. Code paths guarded.
//!   2. Flag flipped to `true` by default once the feature passes UAT.
//!   3. Flag removed once the feature has shipped for a full release cycle.
//!      Removing the flag deletes both the field here and every `if flags.X`
//!      check; the user setting (if any) becomes a no-op and is cleaned up
//!      in a settings migration.
//!
//! Frontend mirror lives at `src/lib/features.ts`.

use serde::Serialize;

use crate::settings::UserSettings;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlags {
    /// Run the streaming preview worker during recording, emitting live
    /// partial captions to the overlay. Currently user-toggleable in
    /// Settings. Default off for less CPU + fewer moving parts on stop.
    pub streaming_preview: bool,

    /// Phase 3 — accepts audio file drops and transcribes them through the
    /// job queue. Off until the file_import code path lands and is verified.
    pub file_import: bool,

    /// Phase 3 — run multiple file imports in parallel. Off until the job
    /// queue's concurrency story is tuned.
    pub batch_transcription: bool,

    /// Phase 5 — persistent transcription history pane. Off until storage
    /// layer (sqlite via the magpie-core trait) lands.
    pub transcription_history: bool,

    /// Phase 3 — "Export vocabulary…" menu entry to save the learned
    /// corrections list as a JSON file. Off until the export UI is built.
    pub vocabulary_export: bool,
}

impl Default for FeatureFlags {
    /// Safe-by-default: every Phase-3+ feature starts off. Anything visible
    /// today (streaming preview) is opt-in via user settings, not the
    /// compile-time default.
    fn default() -> Self {
        Self {
            streaming_preview: false,
            file_import: false,
            batch_transcription: false,
            transcription_history: false,
            vocabulary_export: false,
        }
    }
}

impl FeatureFlags {
    /// Resolve the active feature set: user settings, then environment
    /// overrides. Pure function; the only impure inputs are env reads
    /// pulled in once at the start.
    pub fn resolve(settings: &UserSettings) -> Self {
        let mut flags = Self {
            streaming_preview: settings.streaming_preview,
            ..Self::default()
        };
        flags.apply_env_overrides();
        flags
    }

    fn apply_env_overrides(&mut self) {
        // Each flag below corresponds to MAGPIE_FEATURE_<NAME>. Adding a new
        // flag means appending one line — the field name stays single-source-
        // of-truth and the env var name is mechanical.
        env_override("STREAMING_PREVIEW", &mut self.streaming_preview);
        env_override("FILE_IMPORT", &mut self.file_import);
        env_override("BATCH_TRANSCRIPTION", &mut self.batch_transcription);
        env_override("TRANSCRIPTION_HISTORY", &mut self.transcription_history);
        env_override("VOCABULARY_EXPORT", &mut self.vocabulary_export);
    }
}

/// `MAGPIE_FEATURE_<name>` env-var reader. Accepts `1`, `true`, `yes`, `on`
/// for true; `0`, `false`, `no`, `off` for false. Anything else is a warn-
/// and-ignore (the existing flag value sticks). Empty string is also ignored.
fn env_override(name: &str, target: &mut bool) {
    let var_name = format!("MAGPIE_FEATURE_{name}");
    // Err(_) means the var is not set; keep current value.
    if let Ok(v) = std::env::var(&var_name) {
        match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => *target = true,
            "0" | "false" | "no" | "off" => *target = false,
            "" => {}
            other => {
                log::warn!("{var_name}={other:?} is not a recognized boolean — ignoring",);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env mutations are process-global; serialize them so parallel tests
    // don't clobber each other. The tests are tiny; locking is fine.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // Save current values
        let saved: Vec<_> = vars
            .iter()
            .map(|(k, _)| (k.to_string(), std::env::var(k).ok()))
            .collect();
        for (k, v) in vars {
            match v {
                Some(value) => std::env::set_var(k, value),
                None => std::env::remove_var(k),
            }
        }
        f();
        // Restore
        for (k, v) in saved {
            match v {
                Some(value) => std::env::set_var(&k, value),
                None => std::env::remove_var(&k),
            }
        }
    }

    #[test]
    fn default_is_all_off() {
        let f = FeatureFlags::default();
        assert!(!f.streaming_preview);
        assert!(!f.file_import);
        assert!(!f.batch_transcription);
        assert!(!f.transcription_history);
        assert!(!f.vocabulary_export);
    }

    #[test]
    fn resolve_uses_settings_streaming_preview() {
        let settings = UserSettings {
            streaming_preview: true,
            ..UserSettings::default()
        };
        let f = FeatureFlags::resolve(&settings);
        assert!(f.streaming_preview);
        assert!(!f.file_import, "other flags untouched by settings");
    }

    #[test]
    fn env_override_turns_flag_on() {
        with_env(&[("MAGPIE_FEATURE_FILE_IMPORT", Some("1"))], || {
            let f = FeatureFlags::resolve(&UserSettings::default());
            assert!(f.file_import);
        });
    }

    #[test]
    fn env_override_turns_flag_off() {
        // streaming_preview is true in settings, but env forces it off.
        with_env(&[("MAGPIE_FEATURE_STREAMING_PREVIEW", Some("off"))], || {
            let settings = UserSettings {
                streaming_preview: true,
                ..UserSettings::default()
            };
            let f = FeatureFlags::resolve(&settings);
            assert!(!f.streaming_preview);
        });
    }

    #[test]
    fn env_override_ignores_garbage_and_warns() {
        with_env(
            &[("MAGPIE_FEATURE_VOCABULARY_EXPORT", Some("maybe"))],
            || {
                let f = FeatureFlags::resolve(&UserSettings::default());
                assert!(
                    !f.vocabulary_export,
                    "garbage value leaves the flag at its default"
                );
            },
        );
    }

    #[test]
    fn json_serializes_with_camelcase() {
        let f = FeatureFlags {
            file_import: true,
            ..FeatureFlags::default()
        };
        let s = serde_json::to_string(&f).unwrap();
        assert!(s.contains("\"fileImport\":true"));
        assert!(s.contains("\"streamingPreview\":false"));
        assert!(s.contains("\"vocabularyExport\":false"));
    }
}
