//! Errors produced by profiles load, save, and migration.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfilesError {
    #[error("could not determine app data directory for profiles")]
    NoDataDir,

    #[error("profiles file io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse profiles JSON: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("profiles migration {from_version}→{to_version} failed: {reason}")]
    Migration {
        from_version: u32,
        to_version: u32,
        reason: String,
    },

    #[error("profiles version {found} is newer than supported maximum {supported}")]
    VersionTooNew { found: u32, supported: u32 },

    #[error("profile not found: {id}")]
    NotFound { id: String },

    #[error("invalid profile: {reason}")]
    Invalid { reason: String },
}

pub type Result<T> = std::result::Result<T, ProfilesError>;
