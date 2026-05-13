//! Errors produced by transcript-history load, save, and parse.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HistoryError {
    /// `directories::ProjectDirs` could not resolve a writable app-data dir.
    #[error("could not determine app data directory for history")]
    NoDataDir,

    /// I/O error reading or writing the history JSON file.
    #[error("history file io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// JSON parsing failed. The loader keeps the original on disk and falls
    /// back to an empty ring rather than overwriting; this variant is
    /// returned from explicit `parse`-style helpers used by tests.
    #[error("failed to parse history JSON: {0}")]
    Parse(#[from] serde_json::Error),

    /// The stored `version` field exceeds the highest version this build
    /// knows how to read. We refuse rather than risk losing data.
    #[error("history version {found} is newer than supported maximum {supported}")]
    VersionTooNew { found: u32, supported: u32 },
}

pub type Result<T> = std::result::Result<T, HistoryError>;
