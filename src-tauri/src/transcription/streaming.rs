//! Streaming preview worker.
//!
//! Spawned by `start_recording`, torn down by `stop_recording`. While the
//! user is dictating, this loops every ~300 ms, pulls just the audio that
//! arrived since the previous tick (a cursor into the ring buffer),
//! resamples that chunk incrementally onto a persistent target-rate buffer,
//! runs a cheap (`PartialPreview`) decode over the trailing
//! [`PARTIAL_WINDOW_SECS`] of it, and emits a `partial-transcription` event
//! the overlay can render as a live caption. Fetch, resample, and decode
//! are all O(window) per tick — not O(recording length) — so partials stay
//! fast over long dictations. The final, paste-quality decode still happens
//! only on stop, in `commands.rs`, over the full clip.
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

use crate::audio::resample::StreamingResampler;
use crate::constants::PARTIAL_WINDOW_SECS;
use crate::events::{self, event_names};
use crate::state::{lock_or_recover, AppState};

use super::backend::{CancellationToken, TranscribeMode, TranscribeOptions};

const PARTIAL_INTERVAL_MS: u64 = 300;
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

/// After this many consecutive empty decodes, log a one-shot warning. The
/// streaming worker is silently producing no captions — usually a sign that
/// whisper.cpp's PartialPreview path is failing in a way that returns
/// `Ok(empty)` rather than `Err` (e.g. the macOS CoreML/Metal encoder edge
/// case that prompted dropping the abort_callback). Three polls = ~4.5 s
/// of speech with no caption, which is well past "we should have something
/// to show by now".
const EMPTY_DECODE_WARN_THRESHOLD: usize = 3;

async fn run_loop(
    app: AppHandle,
    state: Arc<AppState>,
    cancel: CancellationToken,
    partial_cancel: CancellationToken,
) {
    // Incremental resample pipeline, persistent across ticks:
    // - `src_cursor`: absolute stream position (in `samples_written` units)
    //   up to which native-rate audio has been consumed from the ring buffer.
    // - `resampler`: stateful linear-interp resampler; carries the source
    //   tail across chunk seams so there is no click/dup/skip at boundaries.
    //   Built lazily on the first tick with audio (the target rate comes
    //   from the backend, which may not be loaded yet).
    // - `preview_pcm`: growing target-rate buffer, trimmed each tick to the
    //   trailing decode window so per-tick work and memory stay O(window).
    let mut src_cursor: u64 = 0;
    let mut resampler: Option<StreamingResampler> = None;
    let mut preview_pcm: Vec<f32> = Vec::new();
    let mut tick = Instant::now() + Duration::from_millis(PARTIAL_INTERVAL_MS);
    let mut consecutive_empty: usize = 0;
    let mut empty_warned = false;
    log::info!("Streaming worker spawned");

    loop {
        let now = Instant::now();
        if now < tick {
            sleep(tick - now).await;
        }
        if cancel.is_cancelled() {
            break;
        }
        tick = Instant::now() + Duration::from_millis(PARTIAL_INTERVAL_MS);

        // Clone the backend Arc out under a brief lock; bail if not loaded.
        // The lock is released before inference so cpal callbacks and other
        // backend readers (e.g. final-on-stop) never wait on the decode.
        // Checked before advancing the cursor so no audio is consumed (and
        // then lost) on ticks where there is nothing to decode against.
        let backend = lock_or_recover(&state.backend).clone();
        let Some(backend) = backend else {
            continue;
        };
        let target_rate = backend.capabilities().sample_rate_hz;

        // Read sample rate, then pull just the audio that arrived since the
        // previous tick (brief locks, released before inference). The cursor
        // uses `samples_written` units (monotonic u64) rather than buffer
        // indices, so it stays valid after the ring buffer wraps.
        let sample_rate = *lock_or_recover(&state.capture_sample_rate);
        let (chunk, chunk_start) = lock_or_recover(&state.audio_buffer).snapshot_from(src_cursor);
        if chunk.is_empty() {
            continue;
        }
        if chunk_start > src_cursor {
            // The ring buffer evicted audio we had not consumed yet — only
            // possible if a decode stalled long enough for the buffer to
            // wrap past the cursor. The preview just carries a small seam
            // artifact at the gap; log and move on.
            log::warn!(
                "Partial worker fell behind ring buffer: {} samples dropped",
                chunk_start - src_cursor
            );
        }
        src_cursor = chunk_start + chunk.len() as u64;

        // (Re)build the resampler if either rate changed. Within one
        // recording both are fixed in practice (the cpal stream and the
        // backend outlive the worker); this is a defensive reset, and a
        // rate change makes the accumulated buffer meaningless anyway.
        if resampler
            .as_ref()
            .map(|r| (r.source_rate(), r.target_rate()))
            != Some((sample_rate, target_rate))
        {
            resampler = Some(StreamingResampler::new(sample_rate, target_rate));
            preview_pcm.clear();
        }
        let out = resampler
            .as_mut()
            .expect("resampler initialized above")
            .process(&chunk);
        preview_pcm.extend_from_slice(&out);

        // Keep only the trailing decode window. The caption is transient UI
        // text, so a sliding window is fine — and it bounds both the decode
        // cost and this buffer's memory regardless of recording length.
        trim_to_tail(&mut preview_pcm, PARTIAL_WINDOW_SECS * target_rate as usize);

        if preview_pcm.len() < MIN_TOTAL_SAMPLES_AT_16K {
            continue;
        }
        let resampled = preview_pcm.clone();

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
                consecutive_empty = 0;
                log::info!(
                    "Partial worker emitting: \"{}\" ({}ms)",
                    out.text,
                    out.duration_ms
                );
                events::emit_event(
                    &app,
                    event_names::PARTIAL_TRANSCRIPTION,
                    PartialTranscriptionPayload {
                        partial: out.text,
                        is_final: false,
                    },
                );
            }
            Ok(Ok(_)) => {
                consecutive_empty += 1;
                if !empty_warned && consecutive_empty >= EMPTY_DECODE_WARN_THRESHOLD {
                    empty_warned = true;
                    log::warn!(
                        "Partial worker: {} consecutive empty decodes — captions will not appear. \
                         Likely whisper.cpp PartialPreview returning empty (CoreML/Metal encoder \
                         edge case on macOS). Final-mode transcription on stop is unaffected.",
                        consecutive_empty
                    );
                }
            }
            Ok(Err(e)) => log::warn!("Partial transcribe failed: {}", e),
            Err(e) => log::warn!("Partial task panicked: {}", e),
        }
    }

    log::info!("Streaming worker exiting");
}

/// Trim `buf` in place so it holds at most its trailing `max_samples`.
/// The drain is a single memmove of at most the window size — trivial next
/// to the decode it bounds.
fn trim_to_tail(buf: &mut Vec<f32>, max_samples: usize) {
    if buf.len() > max_samples {
        let excess = buf.len() - max_samples;
        buf.drain(..excess);
    }
}

// Compile-time regression guards: zeroing these would make the worker either
// spin hot (PARTIAL_INTERVAL_MS=0) or emit on the first cpal callback
// (MIN_TOTAL_SAMPLES_AT_16K=0). Using `const _` instead of a #[test] because
// these are compile-time constants — clippy correctly objects to runtime
// assert!(true) on them.
const _: () = assert!(PARTIAL_INTERVAL_MS >= 100);
const _: () = assert!(MIN_TOTAL_SAMPLES_AT_16K >= 8_000);
// Shrinking the decode window below the minimum-audio gate would trim
// `preview_pcm` under MIN_TOTAL_SAMPLES_AT_16K forever and no partial would
// ever be emitted (gate assumes the 16 kHz whisper rate, the only backend
// rate today).
const _: () = assert!(PARTIAL_WINDOW_SECS * 16_000 >= MIN_TOTAL_SAMPLES_AT_16K);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_to_tail_noop_when_shorter_than_window() {
        let mut buf = vec![1.0, 2.0, 3.0];
        trim_to_tail(&mut buf, 5);
        assert_eq!(buf, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn trim_to_tail_noop_at_exact_window() {
        let mut buf = vec![1.0, 2.0, 3.0];
        trim_to_tail(&mut buf, 3);
        assert_eq!(buf, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn trim_to_tail_keeps_most_recent_samples() {
        let mut buf: Vec<f32> = (0..10).map(|i| i as f32).collect();
        trim_to_tail(&mut buf, 4);
        assert_eq!(buf, vec![6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn trim_to_tail_empty_buffer() {
        let mut buf: Vec<f32> = Vec::new();
        trim_to_tail(&mut buf, 4);
        assert!(buf.is_empty());
    }

    #[test]
    fn trim_to_tail_zero_window_clears() {
        let mut buf = vec![1.0, 2.0];
        trim_to_tail(&mut buf, 0);
        assert!(buf.is_empty());
    }
}
