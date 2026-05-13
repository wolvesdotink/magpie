//! Errors produced by styles load, save, and migration.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StylesError {
    #[error("could not determine app data directory for styles")]
    NoDataDir,

    #[error("styles file io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse styles JSON: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("styles migration {from_version}→{to_version} failed: {reason}")]
    Migration {
        from_version: u32,
        to_version: u32,
        reason: String,
    },

    #[error("styles version {found} is newer than supported maximum {supported}")]
    VersionTooNew { found: u32, supported: u32 },

    #[error("style not found: {id}")]
    NotFound { id: String },

    #[error("style '{name}' is built-in and cannot be deleted")]
    BuiltinDelete { name: String },

    #[error("style is in use by profiles: {profile_names:?}")]
    StyleInUse { profile_names: Vec<String> },

    #[error("invalid style: {reason}")]
    Invalid { reason: String },
}

pub type Result<T> = std::result::Result<T, StylesError>;
