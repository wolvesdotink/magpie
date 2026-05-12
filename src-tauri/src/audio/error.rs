//! Errors produced by the audio capture and resampling pipelines.
//!
//! Categorical (not stringly-typed) so the orchestration layer can react
//! distinctly to "no device" vs. "permission denied" vs. "unsupported
//! sample format" without parsing strings.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    /// `cpal::default_host().default_input_device()` returned `None`. Either
    /// no input device is attached or macOS has revoked microphone access
    /// (see `accessibility::request_microphone_access`).
    #[error("no input device available — check microphone permissions and connected hardware")]
    NoInputDevice,

    /// `device.default_input_config()` failed. Rare; usually means the
    /// selected device disappeared between enumeration and config query.
    #[error("could not determine default input config: {0}")]
    DefaultConfigUnavailable(#[from] cpal::DefaultStreamConfigError),

    /// `device.build_input_stream` rejected our stream parameters.
    #[error("failed to build audio stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),

    /// `Stream::play` failed.
    #[error("failed to start audio stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),

    /// The device reports a sample format we don't know how to decode. cpal
    /// can grow new variants; we explicitly enumerate F32/I16/U16 and reject
    /// anything else rather than silently produce garbage audio.
    #[error("unsupported sample format: {0:?}")]
    UnsupportedSampleFormat(cpal::SampleFormat),
}

pub type Result<T> = std::result::Result<T, AudioError>;
