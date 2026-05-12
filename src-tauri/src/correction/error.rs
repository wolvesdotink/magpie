//! Errors produced by the llama.cpp-backed self-correction engine.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CorrectionError {
    /// `LlamaBackend::init` failed. Should never happen in practice; if it
    /// does, the entire correction subsystem is unavailable until restart.
    #[error("failed to initialize llama backend: {0}")]
    BackendInit(String),

    /// `LlamaModel::load_from_file` rejected the GGUF on disk. Usually means
    /// the file is corrupt or a model from an incompatible llama.cpp version.
    #[error("failed to load correction model at {path}: {message}")]
    ModelLoad { path: PathBuf, message: String },

    /// Inference itself failed — `LlamaContext::eval`, batch overflow, etc.
    #[error("correction inference failed: {0}")]
    Inference(String),

    /// We were asked to run correction but no model is loaded. The frontend
    /// is supposed to gate this with a model selection, but the backend
    /// double-checks defensively.
    #[error("correction model not loaded")]
    NotLoaded,
}

pub type Result<T> = std::result::Result<T, CorrectionError>;
