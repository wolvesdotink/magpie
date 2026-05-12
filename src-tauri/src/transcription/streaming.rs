//! Streaming preview worker.
//!
//! Spawned by `start_recording`, torn down by `stop_recording`. While the
//! user is dictating, this loops every ~1.5s, snapshots the growing audio
//! buffer, runs a cheap (`PartialPreview`) decode through whatever backend
//! is loaded, and emits a `partial-transcription` event the overlay can
//! render as a live caption. The final, paste-quality decode still happens
//! only on stop, in `commands.rs`.
//!
//! Cancellation is two-tiered: `partial_cancel` aborts the in-flight
//! whisper.cpp call (via the abort_callback wired in `whisper_backend.rs`),
//! and `cancel` tells the loop to exit. `stop_recording` flips both so
//! a stale partial can never race the final pass.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;
use tokio::time::{sleep, Instant};

use crate::audio;
use crate::events::{self, event_names};
use crate::state::{lock_or_recover, AppState};

use super::backend::{CancellationToken, TranscribeMode, TranscribeOptions};

const PARTIAL_INTERVAL_MS: u64 = 1500;
/// Skip partials until we have at least 1s of audio at 16kHz. Whisper hates
/// very short clips and tends to hallucinate on them.
const MIN_TOTAL_SAMPLES_AT_16K: usize = 16_000;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialTranscriptionPayload {
    pub partial: String,
    /// Reserved for future use (e.g. when we add stable-hypothesis tracking).
    /// Always false today.
    pub is_final: bool,
}

pub struct StreamingHandle {
    pub cancel: CancellationToken,
    pub partial_cancel: CancellationToken,
    pub join: tauri::async_runtime::JoinHandle<()>,
}

pub fn spawn_streaming_worker(app: AppHandle, state: Arc<AppState>) -> StreamingHandle {
    let cancel = CancellationToken::new();
    let partial_cancel = CancellationToken::new();

    let cancel_clone = cancel.clone();
    let partial_cancel_clone = partial_cancel.clone();

    let join = tauri::async_runtime::spawn(async move {
        run_loop(app, state, cancel_clone, partial_cancel_clone).await;
    });

    StreamingHandle {
        cancel,
        partial_cancel,
        join,
    }
}

async fn run_loop(
    app: AppHandle,
    state: Arc<AppState>,
    cancel: CancellationToken,
    partial_cancel: CancellationToken,
) {
    let mut last_processed_samples: u64 = 0;
    let mut tick = Instant::now() + Duration::from_millis(PARTIAL_INTERVAL_MS);

    loop {
        let now = Instant::now();
        if now < tick {
            sleep(tick - now).await;
        }
        if cancel.is_cancelled() {
            break;
        }
        tick = Instant::now() + Duration::from_millis(PARTIAL_INTERVAL_MS);

        // Snapshot under a brief lock; skip the cycle if no new audio since
        // last decode. Uses samples_written (monotonic u64) rather than len()
        // (plateaus at capacity) so the check still works after the ring
        // buffer wraps.
        let current_samples = lock_or_recover(&state.audio_buffer).samples_written();
        if current_samples == last_processed_samples {
            continue;
        }
        last_processed_samples = current_samples;

        // Read sample rate, then snapshot the buffer (releasing the lock
        // before inference). ~30 s of f32 at 48 kHz is < 6 MB / sub-ms.
        let sample_rate = *lock_or_recover(&state.capture_sample_rate);
        let raw = lock_or_recover(&state.audio_buffer).snapshot();

        // Clone the backend Arc out under a brief lock; bail if not loaded.
        // The lock is released before inference so cpal callbacks and other
        // backend readers (e.g. final-on-stop) never wait on the decode.
        let backend = lock_or_recover(&state.backend).clone();
        let Some(backend) = backend else {
            continue;
        };
        let target_rate = backend.capabilities().sample_rate_hz;
        let resampled = audio::resample::resample(&raw, sample_rate, target_rate);
        if resampled.len() < MIN_TOTAL_SAMPLES_AT_16K {
            continue;
        }

        let language = lock_or_recover(&state.settings).language.clone();

        if partial_cancel.is_cancelled() {
            break;
        }

        let pc = partial_cancel.clone();
        let backend_clone = backend.clone();
        let lang_owned = language.clone();
        let result = tokio::task::spawn_blocking(move || {
            let opts = TranscribeOptions {
                language: lang_owned.as_deref(),
                initial_prompt: None,
                mode: TranscribeMode::PartialPreview,
            };
            backend_clone.transcribe(&resampled, &opts, &pc)
        })
        .await;

        // If we were stopped while inference was running, drop the result —
        // never emit a stale partial after recording-stopped fired.
        if cancel.is_cancelled() || partial_cancel.is_cancelled() {
            break;
        }

        match result {
            Ok(Ok(out)) if !out.text.is_empty() => {
                events::emit_event(
                    &app,
                    event_names::PARTIAL_TRANSCRIPTION,
                    PartialTranscriptionPayload {
                        partial: out.text,
                        is_final: false,
                    },
                );
            }
            Ok(Ok(_)) => {} // empty text — skip emit
            Ok(Err(e)) => log::warn!("Partial transcribe failed: {}", e),
            Err(e) => log::warn!("Partial task panicked: {}", e),
        }
    }

    log::debug!("Streaming worker exiting");
}

// Compile-time regression guards: zeroing these would make the worker either
// spin hot (PARTIAL_INTERVAL_MS=0) or emit on the first cpal callback
// (MIN_TOTAL_SAMPLES_AT_16K=0). Using `const _` instead of a #[test] because
// these are compile-time constants — clippy correctly objects to runtime
// assert!(true) on them.
const _: () = assert!(PARTIAL_INTERVAL_MS >= 500);
const _: () = assert!(MIN_TOTAL_SAMPLES_AT_16K >= 8_000);
