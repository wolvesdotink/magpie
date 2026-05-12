//! Serializable error type that crosses the Tauri command boundary.
//!
//! Every Tauri command in `commands.rs` returns `Result<T, CommandError>`
//! (replacing the previous `Result<T, String>`). The frontend mirror in
//! `src/lib/errors.ts` keeps the discriminant intact so UI code can switch
//! on `kind` instead of pattern-matching free-form strings.
//!
//! Conversions from every per-module error are provided via `From` impls so
//! command handlers can `?`-bubble module errors without manual `map_err`.

use serde::Serialize;
use thiserror::Error;

/// Categorized error returned to the frontend.
///
/// Serialization shape (camelCase):
/// ```json
/// { "kind": "modelNotFound", "message": "unknown model id: foo", "details": null }
/// ```
///
/// `details` is reserved for optional structured context. Today every variant
/// produces a string `message`; future variants can add typed details fields
/// when callers want richer UI affordances (e.g. retry buttons).
#[derive(Debug, Error, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CommandError {
    /// Generic failure that doesn't fit a more specific variant. Use sparingly;
    /// every new use is a hint that a new variant might be earning its keep.
    #[error("{message}")]
    Other {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },

    /// I/O failure (filesystem, disk full, permission denied at fs level).
    #[error("io error: {message}")]
    Io { message: String },

    /// Network failure during a model download or update fetch.
    #[error("network error: {message}")]
    Network { message: String },

    /// Audio capture device unavailable or permissions missing.
    #[error("audio device error: {message}")]
    AudioDevice { message: String },

    /// Transcription backend isn't loaded (no model selected, model load failed).
    #[error("no transcription backend loaded")]
    BackendNotLoaded,

    /// Transcription itself failed mid-decode.
    #[error("transcription failed: {message}")]
    Transcription { message: String },

    /// User-cancelled operation (Esc during recording, cancel-download click).
    #[error("operation cancelled")]
    Cancelled,

    /// The requested model id is not in the registry, or its file is missing.
    #[error("model not found: {model_id}")]
    ModelNotFound { model_id: String },

    /// Self-correction subsystem failure (llama backend / model load / inference).
    #[error("correction error: {message}")]
    Correction { message: String },

    /// Clipboard write or synthetic-paste keystroke failure.
    #[error("output error: {message}")]
    Output { message: String },

    /// Permission required to perform this command is not granted at the OS
    /// level (microphone, accessibility, input monitoring).
    ///
    /// Reserved for future use — today the permission probes return raw
    /// booleans rather than `Result`s, so this variant has no construction
    /// site yet. Kept in the wire schema so consumers (frontend `errors.ts`)
    /// can ship the discriminant before the first per-command migration.
    #[allow(dead_code)]
    #[error("permission denied: {permission}")]
    PermissionDenied { permission: String },

    /// Settings file is corrupt, unmigratable, or otherwise unusable.
    #[error("settings error: {message}")]
    Settings { message: String },

    /// Caller passed an argument that didn't make sense (bad enum string,
    /// out-of-range integer). Distinct from `Other` so the UI can flag the
    /// input field that errored.
    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },
}

impl CommandError {
    /// Shorthand for the catch-all `Other` variant.
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other {
            message: msg.into(),
            details: None,
        }
    }
}

// --- From impls so `?` works across module boundaries. ---

impl From<anyhow::Error> for CommandError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other {
            message: e.to_string(),
            details: e.source().map(|s| s.to_string()),
        }
    }
}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        Self::Io {
            message: e.to_string(),
        }
    }
}

impl From<crate::audio::AudioError> for CommandError {
    fn from(e: crate::audio::AudioError) -> Self {
        use crate::audio::AudioError as A;
        match e {
            A::NoInputDevice => Self::AudioDevice {
                message: "no input device available".into(),
            },
            A::UnsupportedSampleFormat(fmt) => Self::AudioDevice {
                message: format!("unsupported sample format: {:?}", fmt),
            },
            other => Self::AudioDevice {
                message: other.to_string(),
            },
        }
    }
}

impl From<crate::models::ModelError> for CommandError {
    fn from(e: crate::models::ModelError) -> Self {
        use crate::models::ModelError as M;
        match e {
            M::UnknownId(id) => Self::ModelNotFound { model_id: id },
            M::Cancelled => Self::Cancelled,
            M::Network(inner) => Self::Network {
                message: inner.to_string(),
            },
            M::Io { path, source } => Self::Io {
                message: format!("{}: {}", path.display(), source),
            },
            other => Self::Other {
                message: other.to_string(),
                details: None,
            },
        }
    }
}

impl From<crate::output::OutputError> for CommandError {
    fn from(e: crate::output::OutputError) -> Self {
        Self::Output {
            message: e.to_string(),
        }
    }
}

impl From<crate::correction::CorrectionError> for CommandError {
    fn from(e: crate::correction::CorrectionError) -> Self {
        Self::Correction {
            message: e.to_string(),
        }
    }
}

impl From<crate::settings::SettingsError> for CommandError {
    fn from(e: crate::settings::SettingsError) -> Self {
        Self::Settings {
            message: e.to_string(),
        }
    }
}

impl From<crate::transcription::backend::TranscribeError> for CommandError {
    fn from(e: crate::transcription::backend::TranscribeError) -> Self {
        use crate::transcription::backend::TranscribeError as T;
        match e {
            T::NotLoaded => Self::BackendNotLoaded,
            T::Cancelled => Self::Cancelled,
            T::Other(inner) => Self::Transcription {
                message: inner.to_string(),
            },
        }
    }
}

/// Convenience alias for command handlers that want `crate::command_error::Result<T>`
/// in place of the full `Result<T, CommandError>`. The fully-qualified form is
/// used throughout `commands/*.rs` to match the existing house style, so this
/// alias is currently unused — kept available for future consumers.
#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, CommandError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camelcase_kind() {
        let e = CommandError::ModelNotFound {
            model_id: "ghost".into(),
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(
            json.contains("\"kind\":\"modelNotFound\""),
            "unexpected json: {json}"
        );
        assert!(
            json.contains("\"modelId\":\"ghost\""),
            "field renamed to camelCase: {json}"
        );
    }

    #[test]
    fn other_omits_details_when_none() {
        let e = CommandError::other("oops");
        let json = serde_json::to_string(&e).unwrap();
        assert!(
            !json.contains("\"details\""),
            "details should be skipped: {json}"
        );
    }

    #[test]
    fn anyhow_converts_with_message() {
        let e: CommandError = anyhow::anyhow!("nope").into();
        match e {
            CommandError::Other { message, .. } => assert_eq!(message, "nope"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn cancelled_round_trips() {
        let json = serde_json::to_string(&CommandError::Cancelled).unwrap();
        assert!(json.contains("\"kind\":\"cancelled\""));
    }

    #[test]
    fn permission_denied_carries_permission_name() {
        let e = CommandError::PermissionDenied {
            permission: "microphone".into(),
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"permission\":\"microphone\""));
    }
}
