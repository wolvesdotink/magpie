//! Errors produced by user-settings load, save, and migration.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    /// `directories::ProjectDirs` could not resolve a writable app-data dir.
    #[error("could not determine app data directory for settings")]
    NoDataDir,

    /// I/O error reading or writing the settings JSON file.
    #[error("settings file io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// JSON parsing failed. The loader keeps the original on disk and falls
    /// back to defaults rather than overwriting; this error is returned only
    /// from explicit `parse`-style helpers used by migrations and tests.
    #[error("failed to parse settings JSON: {0}")]
    Parse(#[from] serde_json::Error),

    /// A specific migration step refused to run because the input shape did
    /// not match what it expected. The shape mismatch usually means a future
    /// version of settings was opened by an older build.
    #[error("settings migration {from_version}→{to_version} failed: {reason}")]
    Migration {
        from_version: u32,
        to_version: u32,
        reason: String,
    },

    /// The stored `version` field exceeds the highest version this build knows
    /// how to migrate from. We refuse to load rather than risk destroying
    /// user data with a partial parse.
    #[error("settings version {found} is newer than supported maximum {supported}")]
    VersionTooNew { found: u32, supported: u32 },
}

pub type Result<T> = std::result::Result<T, SettingsError>;
