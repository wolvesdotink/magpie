//! Forward-only migration framework for the on-disk `settings.json`.
//!
//! The settings file is stored on disk wrapped in a tiny envelope:
//! ```json
//! { "version": 1, "settings": { ... actual UserSettings ... } }
//! ```
//!
//! Older builds (before Phase 1) wrote a bare `UserSettings` JSON document
//! with no `version` field. We treat that shape as **v0** and migrate it to
//! the current shape on load.
//!
//! Adding a new migration is two lines:
//!   1. Push a new `fn migrate_v{N}_to_v{N+1}` into [`migrations()`].
//!   2. Bump [`CURRENT_VERSION`].
//!
//! Migrations operate on `serde_json::Value` (not the typed `UserSettings`)
//! so they keep working even after future fields are added/removed/renamed.
//! The final value is deserialized as the *current* `UserSettings` after all
//! migrations run, so any field the migration didn't touch picks up the
//! current `#[serde(default)]` defaults.

use serde_json::Value;

use super::error::{Result, SettingsError};

/// The shape version this build writes to disk and expects on load.
pub const CURRENT_VERSION: u32 = 2;

/// A single forward migration step. Index `i` in [`migrations`] migrates
/// **from v`i` to v`i+1`**, so a v0 file applies migrations 0, 1, …,
/// `CURRENT_VERSION - 1`.
type Migration = fn(&mut Value) -> Result<()>;

/// All migrations, in order. Index = source version.
pub fn migrations() -> Vec<Migration> {
    vec![
        // v0 → v1: introduce the versioned envelope. The payload shape is
        // identical to the pre-Phase-1 `UserSettings` so this is a no-op
        // transformation; the only thing that changes is the wrapper.
        migrate_v0_to_v1,
        // v1 → v2: introduce `updateChannel` (defaults to "stable"). The
        // field carries `#[serde(default)]`, so deserialization fills in
        // the default for older files automatically — this migration is
        // version-bookkeeping only.
        migrate_v1_to_v2,
    ]
}

fn migrate_v0_to_v1(_value: &mut Value) -> Result<()> {
    // No payload changes for v0 → v1. Future migrations will mutate `value`.
    Ok(())
}

fn migrate_v1_to_v2(_value: &mut Value) -> Result<()> {
    // `updateChannel` is added in v2 with `#[serde(default)]`, so a
    // missing field deserializes to `Stable`. No payload rewrite needed.
    Ok(())
}

/// Run every migration from `from` up to (but not past) [`CURRENT_VERSION`].
/// Returns [`SettingsError::VersionTooNew`] if `from > CURRENT_VERSION`,
/// which happens when an older build opens a settings file written by a
/// newer one.
pub fn run_migrations(from: u32, value: &mut Value) -> Result<()> {
    if from > CURRENT_VERSION {
        return Err(SettingsError::VersionTooNew {
            found: from,
            supported: CURRENT_VERSION,
        });
    }

    for (idx, migration) in migrations().iter().enumerate() {
        let migration_from = idx as u32;
        if migration_from < from {
            continue;
        }
        if let Err(e) = migration(value) {
            return Err(SettingsError::Migration {
                from_version: migration_from,
                to_version: migration_from + 1,
                reason: e.to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn migration_count_matches_current_version() {
        // Each migration moves the version up by exactly 1, so the number of
        // migrations should equal CURRENT_VERSION.
        assert_eq!(
            migrations().len() as u32,
            CURRENT_VERSION,
            "every CURRENT_VERSION bump requires a migration entry"
        );
    }

    #[test]
    fn v0_to_current_is_lossless_no_op_today() {
        let mut v0 = json!({
            "activationMode": "holdFn",
            "language": null,
            "selectedModel": "small.en",
            "autoStart": true,
            "fillerWords": ["um", "uh"],
            "removeFillers": true
        });
        let original = v0.clone();
        run_migrations(0, &mut v0).expect("v0 -> v1 succeeds");
        assert_eq!(v0, original, "v0 -> v1 must not mutate payload today");
    }

    #[test]
    fn current_version_round_trip_is_noop() {
        let mut v = json!({"selectedModel": "base.en"});
        let original = v.clone();
        run_migrations(CURRENT_VERSION, &mut v).expect("no work to do");
        assert_eq!(v, original);
    }

    #[test]
    fn rejects_future_version() {
        let mut v = json!({});
        let err = run_migrations(CURRENT_VERSION + 7, &mut v).unwrap_err();
        match err {
            SettingsError::VersionTooNew { found, supported } => {
                assert_eq!(found, CURRENT_VERSION + 7);
                assert_eq!(supported, CURRENT_VERSION);
            }
            other => panic!("expected VersionTooNew, got {other:?}"),
        }
    }

    #[test]
    fn intermediate_versions_apply_only_relevant_steps() {
        // If a future build adds migrations [v0→v1, v1→v2, v2→v3],
        // calling run_migrations(2, ..) should skip migrations 0 and 1.
        // For now CURRENT_VERSION == 1 so this is a smoke check that
        // run_migrations(1, ..) is a no-op.
        let mut v = json!({"foo": "bar"});
        run_migrations(1, &mut v).expect("no migrations to run from v1");
        assert_eq!(v, json!({"foo": "bar"}));
    }
}
