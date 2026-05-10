use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use cpal::Stream;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use whisper_rs::WhisperContext;

use tokio::sync::mpsc;

use crate::hotkey::FnKeyMonitorHandle;
use crate::recording::RecordingCommand;
use crate::settings::UserSettings;
use crate::vocabulary::Vocabulary;

/// Shared application state managed by Tauri
pub struct AppState {
    /// Whether we are currently recording audio
    pub recording: AtomicBool,
    /// Whether we are currently processing/transcribing
    pub processing: AtomicBool,
    /// Accumulated PCM samples from the microphone (native sample rate)
    pub audio_buffer: Mutex<Vec<f32>>,
    /// The sample rate of the captured audio
    pub capture_sample_rate: Mutex<u32>,
    /// The active cpal input stream (dropped to stop recording)
    pub active_stream: Mutex<Option<Stream>>,
    /// Loaded whisper context (expensive to create, reused across transcriptions)
    pub whisper_context: Mutex<Option<WhisperContext>>,
    /// Path to the currently loaded model
    pub current_model_path: Mutex<Option<PathBuf>>,
    /// The last transcription result
    pub last_transcription: Mutex<String>,
    /// User settings
    pub settings: Mutex<UserSettings>,
    /// Llama backend (initialized once, shared across correction calls)
    pub llama_backend: Mutex<Option<LlamaBackend>>,
    /// Loaded correction model for self-correction (expensive to create, reused)
    pub correction_model: Mutex<Option<LlamaModel>>,
    /// Path to the currently loaded correction model
    pub current_correction_model_path: Mutex<Option<PathBuf>>,
    /// Current RMS amplitude, stored as f32 bits in an AtomicU32.
    /// Written by the cpal callback, read by the amplitude emitter thread.
    pub amplitude_rms: AtomicU32,
    /// Learned vocabulary for correction biasing
    pub vocabulary: Mutex<Vocabulary>,
    /// When true, the next `Focused(false)` event on the main window will
    /// NOT hide the window. Used when we intentionally launch an external
    /// app (e.g. System Preferences) that steals focus.
    pub suppress_hide: AtomicBool,
    /// Handle to the running Fn key monitor thread (for stop-before-restart)
    pub fn_key_monitor: Mutex<Option<FnKeyMonitorHandle>>,
    /// Sender end of the recording command channel (cloned for hotkey restarts)
    pub recording_tx: Mutex<Option<mpsc::UnboundedSender<RecordingCommand>>>,
}

// Safety: Stream is Send but not Sync by default in cpal,
// but we only access it behind a Mutex
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

impl AppState {
    pub fn new() -> Self {
        Self {
            recording: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            audio_buffer: Mutex::new(Vec::new()),
            capture_sample_rate: Mutex::new(44_100),
            active_stream: Mutex::new(None),
            whisper_context: Mutex::new(None),
            current_model_path: Mutex::new(None),
            last_transcription: Mutex::new(String::new()),
            settings: Mutex::new(UserSettings::load()),
            llama_backend: Mutex::new(None),
            correction_model: Mutex::new(None),
            current_correction_model_path: Mutex::new(None),
            amplitude_rms: AtomicU32::new(0),
            vocabulary: Mutex::new(Vocabulary::load()),
            suppress_hide: AtomicBool::new(false),
            fn_key_monitor: Mutex::new(None),
            recording_tx: Mutex::new(None),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }

    pub fn is_processing(&self) -> bool {
        self.processing.load(Ordering::SeqCst)
    }

    pub fn set_recording(&self, val: bool) {
        self.recording.store(val, Ordering::SeqCst);
    }

    pub fn set_processing(&self, val: bool) {
        self.processing.store(val, Ordering::SeqCst);
    }

    pub fn set_amplitude(&self, val: f32) {
        self.amplitude_rms.store(val.to_bits(), Ordering::Relaxed);
    }

    pub fn get_amplitude(&self) -> f32 {
        f32::from_bits(self.amplitude_rms.load(Ordering::Relaxed))
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
