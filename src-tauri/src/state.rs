use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use cpal::Stream;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use parking_lot::{Mutex, MutexGuard};

use tokio::sync::mpsc;

use crate::audio::AudioRingBuffer;
use crate::frontmost_app::FrontmostApp;
use crate::history::History;
use crate::hotkey::FnKeyMonitorHandle;
use crate::profiles::ProfilesStore;
use crate::recording::RecordingCommand;
use crate::settings::UserSettings;
use crate::styles::StylesStore;
use crate::transcription::backend::{CancellationToken, TranscriptionBackend};
use crate::transcription::streaming::StreamingHandle;
use crate::vocabulary::Vocabulary;

/// Shared application state managed by Tauri.
///
/// # Lock-ordering protocol
///
/// Holding more than one [`AppState`] mutex at the same time is rare but
/// occasionally necessary (e.g. settings save while writing a model path).
/// When it happens, locks MUST be acquired in the order listed below.
/// Releasing order is unconstrained; acquisition order alone prevents
/// classic AB-BA deadlocks.
///
/// 0. `model_load_lock` (outermost): held for the duration of a lazy model
///    load or an idle unload (Memory Saver). Serializes those operations so
///    a background preload, a stop-time load, and the idle-unload watchdog
///    can never interleave (which could free a backend another path just
///    stored). Acquired before any rank below; the slow FFI load runs while
///    holding only this lock — the `backend`/`settings` guards are taken
///    briefly, in rank order, to read the selection and store the result.
/// 1. `settings`
/// 2. `backend`, `current_model_path` (always together when both are
///    held; treated as one rank because the backend swap is conceptually
///    a path-and-loaded-object pair)
/// 3. `audio_buffer`
/// 4. `capture_sample_rate`
/// 5. `active_stream`
/// 6. `streaming_handle`
/// 7. `last_transcription`
///    7.5. `history` (recording.rs pushes after `last_transcription`;
///    `update_settings` may trim it. Never co-held with anything below.)
/// 8. `styles`
/// 9. `profiles`
/// 10. `vocabulary`
/// 11. `llama_backend`, `correction_model`, `current_correction_model_path`
///     (locked together when reloading correction models)
/// 12. `active_downloads`
/// 13. `pending_reload`
/// 14. `fn_key_monitor`, `recording_tx`, `current_shortcut` (orchestration
///     handles; rarely held with anything else)
/// 15. `current_recording_app` (set at recording start, read at resolution
///     time; never held with anything else)
///
/// The atomic fields (`recording`, `processing`, `amplitude_rms`,
/// `suppress_hide`) are lock-free and have no ordering requirement.
///
/// If you find yourself needing to acquire two locks in the wrong order:
/// (a) extract the value from the higher-rank lock into a local first,
/// (b) drop that guard before acquiring the lower-rank lock, or
/// (c) update this comment with a justification before merging.
pub struct AppState {
    /// Whether we are currently recording audio
    pub recording: AtomicBool,
    /// Whether we are currently processing/transcribing
    pub processing: AtomicBool,
    /// Accumulated PCM samples from the microphone (native sample rate).
    /// Bounded ring buffer — see [`AudioRingBuffer::MAX_BUFFER_SAMPLES`]. A
    /// recording longer than the cap discards the oldest samples; the
    /// `has_overflowed` flag stays set so the UI can surface a truncation
    /// hint.
    pub audio_buffer: Mutex<AudioRingBuffer>,
    /// The sample rate of the captured audio
    pub capture_sample_rate: Mutex<u32>,
    /// The active cpal input stream (dropped to stop recording)
    pub active_stream: Mutex<Option<Stream>>,
    /// Loaded transcription backend, behind a Mutex of an Arc so the lock
    /// is held only for the brief Arc clone — never across decode. The
    /// streaming-preview worker and the final-on-stop pass each clone the
    /// inner Arc out, then run inference without any lock held. (arc-swap
    /// would be lock-free for reads but rejects unsized trait objects, and
    /// the contention cost of std::sync::Mutex over a single Arc::clone is
    /// negligible at our access pattern: ~1 read per 1.5s during recording
    /// and 1 read per stop, with rare writes on model load.)
    pub backend: Mutex<Option<Arc<dyn TranscriptionBackend>>>,
    /// Path to the currently loaded model
    pub current_model_path: Mutex<Option<PathBuf>>,
    /// Per-recording streaming worker handle. Inserted in start_recording,
    /// taken (cancelled + awaited) in stop_recording.
    pub streaming_handle: Mutex<Option<StreamingHandle>>,
    /// The last transcription result
    pub last_transcription: Mutex<String>,
    /// Bounded ring of recent dictation transcripts, persisted to disk in
    /// `history.json`. Cap is read from `settings.history_max_entries` at
    /// push time (see `commands/recording.rs`).
    pub history: Mutex<History>,
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
    /// Reusable styles library (formatting + correction + custom rules).
    pub styles: Mutex<StylesStore>,
    /// Per-app profiles binding bundle IDs to styles + scoped vocab.
    pub profiles: Mutex<ProfilesStore>,
    /// Snapshot of the frontmost app at `start_recording` time. Cleared on
    /// recording end. Lower rank than vocabulary; written at recording start,
    /// read during transcription resolution and auto-learning attribution.
    pub current_recording_app: Mutex<Option<FrontmostApp>>,
    /// When true, the next `Focused(false)` event on the main window will
    /// NOT hide the window. Used when we intentionally launch an external
    /// app (e.g. System Preferences) that steals focus.
    pub suppress_hide: AtomicBool,
    /// Handle to the running Fn key monitor thread (for stop-before-restart)
    pub fn_key_monitor: Mutex<Option<FnKeyMonitorHandle>>,
    /// Sender end of the recording command channel (cloned for hotkey restarts)
    pub recording_tx: Mutex<Option<mpsc::UnboundedSender<RecordingCommand>>>,
    /// String of the currently-registered global keyboard shortcut. Used by
    /// `update_global_shortcut` to know what to unregister before binding a
    /// new combination. Initialized when `register_global_shortcut` runs.
    pub current_shortcut: Mutex<Option<String>>,
    /// Set by the encoder-backfill path when a CoreML encoder finished
    /// downloading while the user was mid-recording (or processing a final
    /// pass). The next stop_recording flush checks this and reloads the
    /// WhisperContext so subsequent transcriptions pick up ANE acceleration.
    /// Stored as `(model_id, model_path)` so the reload knows what to load.
    pub pending_reload: Mutex<Option<(String, PathBuf)>>,
    /// In-flight model downloads keyed by `model_id`. The `download_model`
    /// command registers a token on entry and removes it on exit; the
    /// `cancel_download` command flips the token's flag, which the streaming
    /// loop in `downloader::download_model` checks each chunk.
    pub active_downloads: Mutex<HashMap<String, CancellationToken>>,
    /// Outermost lock (rank 0) guarding lazy model loads and idle unloads —
    /// see the lock-ordering protocol above. A unit mutex used purely for
    /// mutual exclusion; it carries no data.
    pub model_load_lock: Mutex<()>,
    /// Epoch-millis timestamp of the last dictation activity (recording
    /// start/stop). Read by the Memory Saver idle-unload watchdog to decide
    /// when a resident model has been idle long enough to drop. Lock-free.
    pub last_activity_ms: AtomicU64,
    /// Cached `Update` returned by the most recent `magpie_updater_check`.
    /// `magpie_updater_install` `.take()`s it before calling
    /// `download_and_install`. Cleared on a check that finds no update.
    /// Desktop-only because `tauri_plugin_updater` is not built on
    /// android/ios (see `src-tauri/Cargo.toml`'s target-cfg dependency).
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub pending_update: Mutex<Option<tauri_plugin_updater::Update>>,
}

// `cpal::Stream` contains C function pointers (FnMut callbacks) that are
// neither `Send` nor `Sync`. We only ever read/write the stream through the
// `Mutex` guard, and the callbacks themselves run on cpal's own audio
// thread (never the caller's), so promoting `AppState` to Send + Sync is
// safe in practice. This unsafe impl was present pre-Phase-2 with the
// std::sync version too — parking_lot doesn't change the requirement.
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

/// Current time as epoch milliseconds. Returns 0 if the system clock is
/// before the Unix epoch (not expected); the idle math degrades gracefully.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl AppState {
    pub fn new() -> Self {
        Self {
            recording: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            audio_buffer: Mutex::new(AudioRingBuffer::default()),
            capture_sample_rate: Mutex::new(44_100),
            active_stream: Mutex::new(None),
            backend: Mutex::new(None),
            current_model_path: Mutex::new(None),
            streaming_handle: Mutex::new(None),
            last_transcription: Mutex::new(String::new()),
            history: Mutex::new(History::load()),
            settings: Mutex::new(UserSettings::load()),
            llama_backend: Mutex::new(None),
            correction_model: Mutex::new(None),
            current_correction_model_path: Mutex::new(None),
            amplitude_rms: AtomicU32::new(0),
            vocabulary: Mutex::new(Vocabulary::load()),
            styles: Mutex::new(StylesStore::load()),
            profiles: Mutex::new(ProfilesStore::load()),
            current_recording_app: Mutex::new(None),
            suppress_hide: AtomicBool::new(false),
            fn_key_monitor: Mutex::new(None),
            recording_tx: Mutex::new(None),
            current_shortcut: Mutex::new(None),
            pending_reload: Mutex::new(None),
            active_downloads: Mutex::new(HashMap::new()),
            model_load_lock: Mutex::new(()),
            last_activity_ms: AtomicU64::new(now_ms()),
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            pending_update: Mutex::new(None),
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

    /// Record that dictation activity just happened (recording start or
    /// stop). Resets the Memory Saver idle clock so a model in active use is
    /// never unloaded out from under the user.
    pub fn mark_activity(&self) {
        self.last_activity_ms.store(now_ms(), Ordering::Relaxed);
    }

    /// Seconds elapsed since the last [`mark_activity`](Self::mark_activity).
    pub fn idle_secs(&self) -> u64 {
        let last = self.last_activity_ms.load(Ordering::Relaxed);
        now_ms().saturating_sub(last) / 1000
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

/// Acquire a `Mutex` lock.
///
/// parking_lot mutexes do not poison on panic — a thread that panics while
/// holding the lock just drops the guard, and the next acquirer gets it as
/// usual. So this is a thin wrapper over `parking_lot::Mutex::lock` whose
/// real job is to **document the convention** used throughout the codebase
/// and to give us a single place to add tracing / contention diagnostics
/// later if needed.
///
/// Keep using this helper at every call site rather than `m.lock()`
/// directly: it keeps the call shape uniform and lets us swap behavior in
/// one place if we want timing/instrumentation later.
#[inline]
pub fn lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn lock_or_recover_returns_guard() {
        let m = Mutex::new(42i32);
        let g = lock_or_recover(&m);
        assert_eq!(*g, 42);
    }

    #[test]
    fn parking_lot_mutex_does_not_poison_on_panic() {
        // parking_lot's whole point: a panic while holding the lock does
        // not poison the mutex. The next acquirer sees the post-panic state.
        let m = Arc::new(Mutex::new(0i32));
        let m_clone = Arc::clone(&m);
        let _ = thread::spawn(move || {
            let mut g = m_clone.lock();
            *g = 99;
            panic!("intentional panic; should not poison");
        })
        .join();

        // No `is_poisoned` on parking_lot mutexes — there's nothing to poison.
        let g = lock_or_recover(&m);
        assert_eq!(*g, 99);
    }

    #[test]
    fn appstate_default_is_idle() {
        let s = AppState::default();
        assert!(!s.is_recording());
        assert!(!s.is_processing());
        assert_eq!(s.get_amplitude(), 0.0);
    }

    #[test]
    fn amplitude_round_trips_through_atomic() {
        let s = AppState::default();
        s.set_amplitude(0.5);
        assert!((s.get_amplitude() - 0.5).abs() < f32::EPSILON);
        s.set_amplitude(0.0);
        assert_eq!(s.get_amplitude(), 0.0);
    }
}
