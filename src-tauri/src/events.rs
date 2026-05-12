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

#[cfg(test)]
mod tests {
    //! Wire-format snapshot tests. Every event payload that crosses the
    //! Tauri boundary is mirrored on the frontend in `src/lib/events.ts`.
    //! If you intentionally change a payload shape, update the snapshot
    //! here AND the TypeScript mirror — these tests are the canary that
    //! tells you to do both.

    use super::*;
    use serde_json::json;

    #[test]
    fn transcription_result_shape() {
        let payload = TranscriptionResult {
            text: "hello world".into(),
            duration_ms: 1234,
        };
        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value, json!({ "text": "hello world", "durationMs": 1234 }));
    }

    #[test]
    fn transcription_error_shape() {
        let payload = TranscriptionError {
            error: "no input device".into(),
        };
        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value, json!({ "error": "no input device" }));
    }

    #[test]
    fn model_download_progress_shape() {
        let payload = ModelDownloadProgress {
            model_id: "base.en".into(),
            percent: 42.5,
            bytes_downloaded: 1000,
            total_bytes: 2353,
        };
        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(
            value,
            json!({
                "modelId": "base.en",
                "percent": 42.5,
                "bytesDownloaded": 1000,
                "totalBytes": 2353
            })
        );
    }

    #[test]
    fn app_state_payload_shape() {
        let payload = AppStatePayload {
            recording: true,
            processing: false,
            has_model: true,
            last_transcription: "hi".into(),
        };
        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(
            value,
            json!({
                "recording": true,
                "processing": false,
                "hasModel": true,
                "lastTranscription": "hi"
            })
        );
    }

    #[test]
    fn permissions_payload_shape() {
        let payload = PermissionsPayload {
            microphone: true,
            accessibility: false,
            input_monitoring: true,
        };
        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(
            value,
            json!({
                "microphone": true,
                "accessibility": false,
                "inputMonitoring": true
            })
        );
    }

    #[test]
    fn audio_amplitude_payload_shape() {
        // 0.5 is exactly representable in f32, so the JSON round-trip is
        // lossless. Don't pick e.g. 0.42 here — it's a tenant of decimal
        // not binary and the assertion would chase precision noise.
        let payload = AudioAmplitudePayload { amplitude: 0.5 };
        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value, json!({ "amplitude": 0.5 }));
    }

    #[test]
    fn vocabulary_learned_payload_shape() {
        let payload = VocabularyLearnedPayload {
            wrong: "cubernetes".into(),
            correct: "Kubernetes".into(),
        };
        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(
            value,
            json!({ "wrong": "cubernetes", "correct": "Kubernetes" })
        );
    }

    #[test]
    fn event_name_constants_match_frontend_strings() {
        // The frontend listens on these exact string names. A typo or
        // accidental rename on either side desynchronizes the wire. Each
        // assertion below is the contract.
        assert_eq!(event_names::RECORDING_STARTED, "recording-started");
        assert_eq!(event_names::RECORDING_STOPPED, "recording-stopped");
        assert_eq!(event_names::TRANSCRIPTION_STARTED, "transcription-started");
        assert_eq!(
            event_names::TRANSCRIPTION_COMPLETE,
            "transcription-complete"
        );
        assert_eq!(event_names::TRANSCRIPTION_ERROR, "transcription-error");
        assert_eq!(event_names::PARTIAL_TRANSCRIPTION, "partial-transcription");
        assert_eq!(
            event_names::MODEL_DOWNLOAD_PROGRESS,
            "model-download-progress"
        );
        assert_eq!(
            event_names::MODEL_DOWNLOAD_COMPLETE,
            "model-download-complete"
        );
        assert_eq!(
            event_names::MODEL_DOWNLOAD_CANCELLED,
            "model-download-cancelled"
        );
        assert_eq!(event_names::APP_STATE_CHANGED, "app-state-changed");
        assert_eq!(event_names::PERMISSIONS_STATUS, "permissions-status");
        assert_eq!(event_names::CORRECTION_STARTED, "correction-started");
        assert_eq!(event_names::CORRECTION_COMPLETE, "correction-complete");
        assert_eq!(event_names::AUDIO_AMPLITUDE, "audio-amplitude");
        assert_eq!(event_names::VOCABULARY_LEARNED, "vocabulary-learned");
    }
}
