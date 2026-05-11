use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Event names used for backend -> frontend communication
pub mod event_names {
    pub const RECORDING_STARTED: &str = "recording-started";
    pub const RECORDING_STOPPED: &str = "recording-stopped";
    pub const TRANSCRIPTION_STARTED: &str = "transcription-started";
    pub const TRANSCRIPTION_COMPLETE: &str = "transcription-complete";
    pub const TRANSCRIPTION_ERROR: &str = "transcription-error";
    pub const PARTIAL_TRANSCRIPTION: &str = "partial-transcription";
    pub const MODEL_DOWNLOAD_PROGRESS: &str = "model-download-progress";
    pub const MODEL_DOWNLOAD_COMPLETE: &str = "model-download-complete";
    pub const MODEL_DOWNLOAD_CANCELLED: &str = "model-download-cancelled";
    pub const APP_STATE_CHANGED: &str = "app-state-changed";
    pub const PERMISSIONS_STATUS: &str = "permissions-status";
    pub const CORRECTION_STARTED: &str = "correction-started";
    pub const CORRECTION_COMPLETE: &str = "correction-complete";
    pub const AUDIO_AMPLITUDE: &str = "audio-amplitude";
    pub const VOCABULARY_LEARNED: &str = "vocabulary-learned";
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub duration_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionError {
    pub error: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadProgress {
    pub model_id: String,
    pub percent: f64,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatePayload {
    pub recording: bool,
    pub processing: bool,
    pub has_model: bool,
    pub last_transcription: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsPayload {
    pub microphone: bool,
    /// Accessibility permission — needed by `enigo` to post Cmd+V
    /// keystrokes into the active app after transcription.
    pub accessibility: bool,
    /// Input Monitoring permission — needed by the CGEventTap to *receive*
    /// keyboard events (including Fn via FlagsChanged). On macOS 10.15+
    /// this is a separate TCC service from Accessibility; without it the
    /// tap is created successfully but silently receives no events.
    pub input_monitoring: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioAmplitudePayload {
    /// Normalized amplitude value, 0.0 to 1.0
    pub amplitude: f32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabularyLearnedPayload {
    pub wrong: String,
    pub correct: String,
}

/// Helper to emit typed events to the frontend
pub fn emit_event<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(e) = app.emit(event, payload) {
        log::error!("Failed to emit event '{}': {}", event, e);
    }
}
