//! Errors produced by the clipboard write + paste-keystroke output paths.
//!
//! Currently unused in production paths — Phase 1 scaffolding. See
//! `audio/error.rs` for the migration rationale.

use thiserror::Error;

#[allow(dead_code)] // Phase 1 scaffolding; consumers migrate in a later phase.
#[derive(Debug, Error)]
pub enum OutputError {
    /// `arboard::Clipboard::new` or `set_text` failed. arboard's own error
    /// type is wrapped via `#[from]` so callers can match on the inner cause
    /// if needed.
    #[error("clipboard write failed: {0}")]
    Clipboard(#[from] arboard::Error),

    /// `enigo::Enigo::new` failed. enigo's constructor can fail when the OS
    /// denies Accessibility permission for synthetic input.
    #[error("could not create input simulator (Accessibility permission may be missing): {0}")]
    InputSimInit(String),

    /// `enigo::Keyboard::key`/`text` returned an error. Distinct from
    /// `InputSimInit` so retries can be targeted.
    #[error("synthetic keystroke failed: {0}")]
    Keystroke(String),
}

#[allow(dead_code)] // Phase 1 scaffolding; consumers migrate in a later phase.
pub type Result<T> = std::result::Result<T, OutputError>;
