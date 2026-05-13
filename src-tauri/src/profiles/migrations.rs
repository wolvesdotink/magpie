//! Forward-only migration framework for the on-disk `profiles.json`.

use serde_json::Value;

use super::error::{ProfilesError, Result};

pub const CURRENT_VERSION: u32 = 1;

type Migration = fn(&mut Value) -> Result<()>;

pub fn migrations() -> Vec<Migration> {
    vec![]
}

pub fn run_migrations(from: u32, value: &mut Value) -> Result<()> {
    if from > CURRENT_VERSION {
        return Err(ProfilesError::VersionTooNew {
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
            return Err(ProfilesError::Migration {
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
    fn current_version_round_trip_is_noop() {
        let mut v = json!({"profiles": []});
        let original = v.clone();
        run_migrations(CURRENT_VERSION, &mut v).expect("no work to do");
        assert_eq!(v, original);
    }

    #[test]
    fn rejects_future_version() {
        let mut v = json!({});
        let err = run_migrations(CURRENT_VERSION + 1, &mut v).unwrap_err();
        assert!(matches!(err, ProfilesError::VersionTooNew { .. }));
    }
}
