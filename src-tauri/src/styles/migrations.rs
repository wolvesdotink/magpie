//! Forward-only migration framework for the on-disk `styles.json`.

use serde_json::Value;

use super::error::{Result, StylesError};

pub const CURRENT_VERSION: u32 = 1;

type Migration = fn(&mut Value) -> Result<()>;

pub fn migrations() -> Vec<Migration> {
    vec![
        // No migrations yet — v0 → v1 is a placeholder for the first
        // schema version. Future migrations append here.
    ]
}

pub fn run_migrations(from: u32, value: &mut Value) -> Result<()> {
    if from > CURRENT_VERSION {
        return Err(StylesError::VersionTooNew {
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
            return Err(StylesError::Migration {
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
        let mut v = json!({"styles": []});
        let original = v.clone();
        run_migrations(CURRENT_VERSION, &mut v).expect("no work to do");
        assert_eq!(v, original);
    }

    #[test]
    fn rejects_future_version() {
        let mut v = json!({});
        let err = run_migrations(CURRENT_VERSION + 1, &mut v).unwrap_err();
        match err {
            StylesError::VersionTooNew { found, supported } => {
                assert_eq!(found, CURRENT_VERSION + 1);
                assert_eq!(supported, CURRENT_VERSION);
            }
            other => panic!("expected VersionTooNew, got {other:?}"),
        }
    }
}
