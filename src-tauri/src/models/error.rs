//! Errors produced by the model registry, downloader, and on-disk storage.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    /// The requested model ID is not in the registry. Either a stale settings
    /// file or a typo from the frontend.
    #[error("unknown model id: {0}")]
    UnknownId(String),

    /// `directories::ProjectDirs::from(...)` returned None, meaning the OS
    /// could not provide an app-data directory at all.
    #[error("could not determine app data directory for model storage")]
    NoDataDir,

    /// Filesystem error reading or writing a model file.
    #[error("model file io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// HTTP transport error while downloading a model.
    #[error("download network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Non-2xx HTTP response (download URL invalid, model removed, etc.).
    #[error("download HTTP status {status} for {url}")]
    HttpStatus { status: u16, url: String },

    /// User requested cancellation via `cancel_download`.
    #[error("download cancelled by user")]
    Cancelled,

    /// Failed to unpack a CoreML encoder `.mlmodelc.zip`.
    #[error("failed to extract encoder archive: {0}")]
    UnzipError(#[from] zip::result::ZipError),

    /// Size mismatch between the registry's declared `size_bytes` and what
    /// the server returned. We treat this as fatal so a corrupted download
    /// can't masquerade as a valid model.
    #[error("download size mismatch: expected {expected} bytes, got {actual}")]
    SizeMismatch { expected: u64, actual: u64 },

    /// CoreML encoder package didn't pass post-unpack validation
    /// (missing markers, no `.mlmodelc` directory, malformed path, etc.).
    /// Distinct from `UnzipError` because the unzip succeeded — what came
    /// out the other side was unusable.
    #[error("CoreML encoder validation failed: {0}")]
    EncoderInvalid(String),
}

pub type Result<T> = std::result::Result<T, ModelError>;
