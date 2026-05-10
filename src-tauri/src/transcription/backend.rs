// Intentional public API surface: these fields/variants/methods exist so a
// future second backend (Moonshine, WhisperKit, Parakeet) drops in without
// extending the trait. Suppress dead-code warnings until then.
#![allow(dead_code)]

//! Transcription backend abstraction.
//!
//! Today the only impl is `WhisperBackend` (whisper.cpp via whisper-rs). The
//! trait shape is intentionally minimal so future backends (Moonshine via
//! ONNX, WhisperKit via Swift sidecar, Parakeet, etc.) can drop in without
//! rewiring `commands.rs` or `state.rs`.
//!
//! Key shape notes:
//!   * `&self transcribe`: callers may invoke from multiple threads (the
//!     streaming-preview worker and the final-on-stop pass can briefly
//!     overlap). Each impl is responsible for its own internal serialization
//!     if needed; Whisper satisfies this trivially because it creates a
//!     fresh `WhisperState` per call.
//!   * `Send + Sync + 'static`: lets us store as `Arc<dyn TranscriptionBackend>`
//!     in `AppState` and clone cheaply into `spawn_blocking` closures.
//!   * `CancellationToken`: a sync `AtomicBool` poll, sized for whisper.cpp's
//!     `abort_callback` FFI hook (NOT an async waker). Avoids pulling in
//!     `tokio_util` for a use case that's purely synchronous.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Distinguishes "fast & cheap" partial decodes (overlay preview during
/// recording) from "paste-quality" final decodes (after stop_recording).
/// Backends can ignore the flag if they don't differentiate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscribeMode {
    /// Final, user-visible result. Use full params.
    Final,
    /// Mid-recording preview. Use cheap params, drops are acceptable.
    PartialPreview,
}

/// Inputs to a single transcription call. Borrows so the streaming worker
/// (which runs the same options every cycle) doesn't pay for clones.
pub struct TranscribeOptions<'a> {
    pub language: Option<&'a str>,
    pub initial_prompt: Option<&'a str>,
    pub mode: TranscribeMode,
}

pub struct TranscribeOutput {
    pub text: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct BackendCapabilities {
    /// Required input rate. Whisper = 16_000. Future backends may differ;
    /// the orchestrator (commands.rs, streaming worker) reads this to
    /// resample to the right rate.
    pub sample_rate_hz: u32,
    pub supports_initial_prompt: bool,
    pub supports_partial_decode: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum TranscribeError {
    #[error("backend not loaded")]
    NotLoaded,
    #[error("transcription cancelled")]
    Cancelled,
    #[error("transcription failed: {0}")]
    Other(#[from] anyhow::Error),
}

/// Cheap, cloneable cancellation handle. Wraps an `AtomicBool` behind an
/// `Arc` so the streaming worker can hand the same token to the in-flight
/// `transcribe()` call (wired into whisper.cpp's `abort_callback`) and to
/// `stop_recording`.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    inner: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.inner.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }
}

pub trait TranscriptionBackend: Send + Sync + 'static {
    fn capabilities(&self) -> BackendCapabilities;
    fn name(&self) -> &'static str;

    /// Transcribe a mono f32 buffer at `capabilities().sample_rate_hz`.
    /// Returns whatever segments accumulated; if `cancel` was tripped
    /// mid-decode the result may be partial or empty (caller decides
    /// whether to use it).
    fn transcribe(
        &self,
        audio: &[f32],
        opts: &TranscribeOptions<'_>,
        cancel: &CancellationToken,
    ) -> Result<TranscribeOutput, TranscribeError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_token_signals() {
        let t = CancellationToken::new();
        assert!(!t.is_cancelled());
        let t2 = t.clone();
        t2.cancel();
        assert!(t.is_cancelled());
    }
}
