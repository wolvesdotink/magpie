//! Bounded ring buffer for captured PCM audio.
//!
//! Replaces the unbounded `Vec<f32>` that backed `AppState::audio_buffer`
//! pre-Phase-2. A 60-minute dictation at 48 kHz would have grown that Vec
//! to ~700 MB; the ring buffer caps memory at [`MAX_BUFFER_SAMPLES`] and
//! drops the oldest samples on overflow.
//!
//! On overflow the [`has_overflowed`](AudioRingBuffer::has_overflowed) flag
//! is set so a higher-rank layer can surface a "recording truncated" UI
//! cue. The buffer itself stays usable — partial / final decode just sees
//! the most recent [`MAX_BUFFER_SAMPLES`] samples.
//!
//! API mirrors the parts of `Vec<f32>` previously used at the call sites
//! (`push_slice` ↔ `extend_from_slice`, `clear`, `len`, `is_empty`,
//! `snapshot` ↔ `clone`) so the swap is local.

use std::collections::VecDeque;

/// Maximum samples retained. 28 800 000 = 10 min at 48 kHz, 30 min at
/// 16 kHz — a comfortable ceiling for either the highest plausible cpal
/// rate or a future downsampled-on-capture buffer. ~115 MB at peak.
///
/// The number is generous on purpose: this is a *runaway-prevention* cap,
/// not a target. 99% of real dictations are well under 1 min.
pub const MAX_BUFFER_SAMPLES: usize = 28_800_000;

#[derive(Debug)]
pub struct AudioRingBuffer {
    data: VecDeque<f32>,
    capacity: usize,
    samples_written: u64,
    overflowed: bool,
}

impl Default for AudioRingBuffer {
    fn default() -> Self {
        Self::with_capacity(MAX_BUFFER_SAMPLES)
    }
}

impl AudioRingBuffer {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            // VecDeque::with_capacity reserves but does not commit physical
            // pages — they get committed lazily on first write. Allocating
            // the worst case up front is fine.
            data: VecDeque::with_capacity(capacity),
            capacity,
            samples_written: 0,
            overflowed: false,
        }
    }

    /// Append `samples`, dropping oldest content if the capacity is full.
    ///
    /// Runs inside the real-time cpal callback (under the `audio_buffer`
    /// mutex), so eviction is a single bulk `drain` + `extend` rather than
    /// a per-sample pop/push loop.
    pub fn push_slice(&mut self, samples: &[f32]) {
        // A slice larger than the whole buffer: only its tail can survive.
        // The discarded head counts as overflow just like evicted content.
        let src = if samples.len() > self.capacity {
            self.overflowed = true;
            &samples[samples.len() - self.capacity..]
        } else {
            samples
        };
        // Evict exactly as many of the oldest samples as the new content
        // needs, in one drain.
        let overflow = (self.data.len() + src.len()).saturating_sub(self.capacity);
        if overflow > 0 {
            self.data.drain(..overflow);
            self.overflowed = true;
        }
        self.data.extend(src.iter().copied());
        self.samples_written = self.samples_written.saturating_add(samples.len() as u64);
    }

    /// Current logical length (number of samples available for read). Plateaus
    /// at `capacity` after the buffer first overflows.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Total samples ever written into the buffer for this recording session.
    /// Unlike `len()`, this keeps growing past `capacity` — the streaming
    /// worker uses it to detect "new audio since last decode" even after
    /// the buffer wrapped.
    pub fn samples_written(&self) -> u64 {
        self.samples_written
    }

    /// `true` once at least one sample has been evicted to make room for a
    /// newer one. Sticky for the life of the recording (until `clear()`).
    pub fn has_overflowed(&self) -> bool {
        self.overflowed
    }

    /// Copy current contents into a flat `Vec<f32>` in chronological order.
    /// Allocates one Vec sized to `len()`. Whisper / partial-decode callers
    /// use this to take a snapshot they can hold across the decode.
    pub fn snapshot(&self) -> Vec<f32> {
        let (a, b) = self.data.as_slices();
        let mut out = Vec::with_capacity(self.data.len());
        out.extend_from_slice(a);
        out.extend_from_slice(b);
        out
    }

    /// Copy the samples at or after absolute stream position `from` (in
    /// [`samples_written`](Self::samples_written) units) in chronological
    /// order, returning them together with the absolute position of the
    /// first returned sample. That position equals `from` in the normal
    /// case; it is greater when the requested range was already evicted by
    /// overflow (caller fell behind by more than the capacity), and clamps
    /// to `samples_written` (empty result) if `from` is in the future.
    ///
    /// The streaming worker uses this to fetch only the audio that arrived
    /// since its previous tick instead of re-snapshotting the whole buffer.
    pub fn snapshot_from(&self, from: u64) -> (Vec<f32>, u64) {
        let oldest = self.samples_written - self.data.len() as u64;
        let start = from.clamp(oldest, self.samples_written);
        let skip = (start - oldest) as usize;
        let (a, b) = self.data.as_slices();
        let mut out = Vec::with_capacity(self.data.len() - skip);
        if skip < a.len() {
            out.extend_from_slice(&a[skip..]);
            out.extend_from_slice(b);
        } else {
            out.extend_from_slice(&b[skip - a.len()..]);
        }
        (out, start)
    }

    /// Reset for a new recording. The underlying allocation is retained.
    pub fn clear(&mut self) {
        self.data.clear();
        self.samples_written = 0;
        self.overflowed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_snapshot_round_trip() {
        let mut r = AudioRingBuffer::with_capacity(10);
        r.push_slice(&[1.0, 2.0, 3.0]);
        assert_eq!(r.len(), 3);
        assert_eq!(r.snapshot(), vec![1.0, 2.0, 3.0]);
        assert!(!r.has_overflowed());
        assert_eq!(r.samples_written(), 3);
    }

    #[test]
    fn overflow_drops_oldest() {
        let mut r = AudioRingBuffer::with_capacity(4);
        r.push_slice(&[1.0, 2.0, 3.0, 4.0]);
        r.push_slice(&[5.0, 6.0]);
        assert_eq!(r.len(), 4);
        assert_eq!(r.snapshot(), vec![3.0, 4.0, 5.0, 6.0]);
        assert!(r.has_overflowed());
        assert_eq!(r.samples_written(), 6);
    }

    #[test]
    fn push_larger_than_capacity_keeps_tail() {
        let mut r = AudioRingBuffer::with_capacity(3);
        r.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(r.len(), 3);
        assert_eq!(r.snapshot(), vec![3.0, 4.0, 5.0]);
        assert!(r.has_overflowed());
        // samples_written counts everything pushed, including the dropped head.
        assert_eq!(r.samples_written(), 5);
    }

    #[test]
    fn oversized_push_into_partially_filled_buffer_keeps_tail() {
        let mut r = AudioRingBuffer::with_capacity(3);
        r.push_slice(&[1.0]);
        r.push_slice(&[2.0, 3.0, 4.0, 5.0]);
        assert_eq!(r.snapshot(), vec![3.0, 4.0, 5.0]);
        assert!(r.has_overflowed());
        assert_eq!(r.samples_written(), 5);
    }

    #[test]
    fn exact_capacity_fill_does_not_overflow() {
        let mut r = AudioRingBuffer::with_capacity(4);
        r.push_slice(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(r.len(), 4);
        assert_eq!(r.snapshot(), vec![1.0, 2.0, 3.0, 4.0]);
        assert!(!r.has_overflowed());
        // One more sample past the exact fill is what trips overflow.
        r.push_slice(&[5.0]);
        assert_eq!(r.snapshot(), vec![2.0, 3.0, 4.0, 5.0]);
        assert!(r.has_overflowed());
    }

    #[test]
    fn snapshot_preserves_chronological_order_after_many_wraps() {
        let mut r = AudioRingBuffer::with_capacity(3);
        for i in 1..=10 {
            r.push_slice(&[i as f32]);
        }
        // After 10 pushes into cap=3, last 3 wins.
        assert_eq!(r.snapshot(), vec![8.0, 9.0, 10.0]);
        assert!(r.has_overflowed());
        assert_eq!(r.samples_written(), 10);
    }

    #[test]
    fn clear_resets_state_but_keeps_allocation() {
        let mut r = AudioRingBuffer::with_capacity(100);
        r.push_slice(&[1.0, 2.0, 3.0]);
        r.clear();
        assert!(r.is_empty());
        assert!(!r.has_overflowed());
        assert_eq!(r.samples_written(), 0);
        // Allocation retained (VecDeque::clear does not shrink) — implicit
        // contract checked by the docstring; we don't assert capacity here
        // because VecDeque doesn't expose a way to read it directly.
    }

    #[test]
    fn empty_push_is_noop() {
        let mut r = AudioRingBuffer::with_capacity(8);
        r.push_slice(&[]);
        assert!(r.is_empty());
        assert_eq!(r.samples_written(), 0);
    }

    #[test]
    fn snapshot_of_partial_fill_is_in_order() {
        let mut r = AudioRingBuffer::with_capacity(10);
        r.push_slice(&[1.0, 2.0]);
        r.push_slice(&[3.0]);
        r.push_slice(&[4.0, 5.0]);
        assert_eq!(r.snapshot(), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn snapshot_from_zero_equals_full_snapshot() {
        let mut r = AudioRingBuffer::with_capacity(10);
        r.push_slice(&[1.0, 2.0, 3.0, 4.0]);
        let (samples, start) = r.snapshot_from(0);
        assert_eq!(samples, r.snapshot());
        assert_eq!(start, 0);
    }

    #[test]
    fn snapshot_from_mid_buffer_returns_suffix() {
        let mut r = AudioRingBuffer::with_capacity(10);
        r.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let (samples, start) = r.snapshot_from(3);
        assert_eq!(samples, vec![4.0, 5.0]);
        assert_eq!(start, 3);
    }

    #[test]
    fn snapshot_from_current_position_is_empty() {
        let mut r = AudioRingBuffer::with_capacity(10);
        r.push_slice(&[1.0, 2.0, 3.0]);
        let (samples, start) = r.snapshot_from(3);
        assert!(samples.is_empty());
        assert_eq!(start, 3);
    }

    #[test]
    fn snapshot_from_future_position_clamps_to_written() {
        let mut r = AudioRingBuffer::with_capacity(10);
        r.push_slice(&[1.0, 2.0]);
        let (samples, start) = r.snapshot_from(99);
        assert!(samples.is_empty());
        assert_eq!(start, 2);
    }

    #[test]
    fn snapshot_from_evicted_position_clamps_to_oldest() {
        // Capacity 4, 7 written: positions 0..3 are gone; a cursor at 1
        // gets the oldest retained sample (position 3) and reports it.
        let mut r = AudioRingBuffer::with_capacity(4);
        r.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        let (samples, start) = r.snapshot_from(1);
        assert_eq!(samples, vec![4.0, 5.0, 6.0, 7.0]);
        assert_eq!(start, 3);
    }

    #[test]
    fn snapshot_from_tracks_incremental_reads_across_wraps() {
        // Simulate the streaming worker's cursor: read everything new after
        // each push, across enough pushes to wrap the buffer several times.
        // The concatenated reads must equal the full stream (nothing
        // duplicated, nothing skipped) as long as the cursor keeps up.
        let mut r = AudioRingBuffer::with_capacity(5);
        let mut cursor: u64 = 0;
        let mut collected = Vec::new();
        for i in 0..20 {
            r.push_slice(&[i as f32, i as f32 + 0.5]);
            let (samples, start) = r.snapshot_from(cursor);
            assert_eq!(start, cursor, "cursor kept up; nothing evicted");
            cursor = start + samples.len() as u64;
            collected.extend(samples);
        }
        let expected: Vec<f32> = (0..20).flat_map(|i| [i as f32, i as f32 + 0.5]).collect();
        assert_eq!(collected, expected);
        assert_eq!(cursor, r.samples_written());
    }

    #[test]
    fn default_uses_max_capacity() {
        let r = AudioRingBuffer::default();
        // Capacity field is private; the observable property is "very large".
        // Push one sample and check it lands without overflow.
        let mut r = r;
        r.push_slice(&[0.42]);
        assert_eq!(r.len(), 1);
        assert!(!r.has_overflowed());
    }
}
