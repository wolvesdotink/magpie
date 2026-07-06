/// Resample audio from `from_rate` to `to_rate` using linear interpolation.
/// Simple but adequate for speech recognition; backend-agnostic so each ASR
/// can declare its own preferred sample rate via `BackendCapabilities`.
pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx_floor = src_idx.floor() as usize;
        let frac = (src_idx - idx_floor as f64) as f32;

        let sample = if idx_floor + 1 < samples.len() {
            samples[idx_floor] * (1.0 - frac) + samples[idx_floor + 1] * frac
        } else if idx_floor < samples.len() {
            samples[idx_floor]
        } else {
            0.0
        };

        output.push(sample);
    }

    output
}

/// Stateful, chunk-at-a-time variant of [`resample`].
///
/// The streaming-preview worker feeds source samples as they arrive instead
/// of re-resampling the whole recording every tick. The concatenated output
/// of successive [`process`](Self::process) calls matches a single
/// [`resample`] call over the concatenated input, except that the last
/// couple of output samples are withheld until the next chunk supplies
/// their right-hand interpolation neighbor (the whole-clip function falls
/// back to sample-and-hold at the clip end; a stream has no end, so we wait
/// for the real neighbor instead).
///
/// Continuity across chunk seams is kept by carrying the still-needed
/// source tail and the absolute output position, so there is no click,
/// duplicated, or skipped sample at a boundary.
pub struct StreamingResampler {
    from_rate: u32,
    to_rate: u32,
    /// Source samples per output sample (`from_rate / to_rate`).
    ratio: f64,
    /// Absolute index in the whole source stream of `tail[0]`.
    tail_start: u64,
    /// Source samples not yet fully consumed by interpolation.
    tail: Vec<f32>,
    /// Output samples emitted so far.
    out_emitted: u64,
}

impl StreamingResampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        Self {
            from_rate,
            to_rate,
            ratio: from_rate as f64 / to_rate as f64,
            tail_start: 0,
            tail: Vec::new(),
            out_emitted: 0,
        }
    }

    pub fn source_rate(&self) -> u32 {
        self.from_rate
    }

    pub fn target_rate(&self) -> u32 {
        self.to_rate
    }

    /// Feed the next source chunk; returns the output samples it unlocked.
    pub fn process(&mut self, chunk: &[f32]) -> Vec<f32> {
        if self.from_rate == self.to_rate {
            return chunk.to_vec();
        }

        self.tail.extend_from_slice(chunk);
        let total_src = self.tail_start + self.tail.len() as u64;
        let mut out = Vec::with_capacity((chunk.len() as f64 / self.ratio) as usize + 2);

        loop {
            // Same arithmetic as `resample` so the outputs are identical:
            // output i interpolates the source at position i * ratio.
            let src_idx = self.out_emitted as f64 * self.ratio;
            let idx_floor = src_idx.floor() as u64;
            // Both neighbors must exist; the seam sample waits for the next
            // chunk rather than being approximated with sample-and-hold.
            if idx_floor + 1 >= total_src {
                break;
            }
            let frac = (src_idx - idx_floor as f64) as f32;
            let rel = (idx_floor - self.tail_start) as usize;
            out.push(self.tail[rel] * (1.0 - frac) + self.tail[rel + 1] * frac);
            self.out_emitted += 1;
        }

        // Drop the prefix the next output position no longer interpolates
        // over; the retained tail stays O(ratio) samples. When downsampling,
        // that position can lie beyond the source received so far (the
        // stride straddles the chunk boundary), so cap the drop at the
        // tail length.
        let next_floor = (self.out_emitted as f64 * self.ratio).floor() as u64;
        let drop = (next_floor.saturating_sub(self.tail_start) as usize).min(self.tail.len());
        if drop > 0 {
            self.tail.drain(..drop);
            self.tail_start += drop as u64;
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `samples` through a `StreamingResampler` in chunks of the given
    /// (cycled) sizes and return the concatenated output.
    fn resample_chunked(samples: &[f32], from: u32, to: u32, chunk_sizes: &[usize]) -> Vec<f32> {
        let mut r = StreamingResampler::new(from, to);
        let mut out = Vec::new();
        let mut pos = 0;
        let mut i = 0;
        while pos < samples.len() {
            let n = chunk_sizes[i % chunk_sizes.len()].min(samples.len() - pos);
            out.extend(r.process(&samples[pos..pos + n]));
            pos += n;
            i += 1;
        }
        out
    }

    /// Chunked output must be a prefix of the whole-clip output (the stream
    /// withholds at most a few trailing samples until more source arrives),
    /// and every shared sample must match within float tolerance.
    fn assert_chunked_matches_whole(chunked: &[f32], whole: &[f32], max_withheld: usize) {
        assert!(
            chunked.len() <= whole.len() && whole.len() - chunked.len() <= max_withheld,
            "length mismatch: chunked {} vs whole {} (max withheld {})",
            chunked.len(),
            whole.len(),
            max_withheld
        );
        for (i, (c, w)) in chunked.iter().zip(whole.iter()).enumerate() {
            assert!(
                (c - w).abs() <= 1e-6,
                "sample {} diverges: chunked {} vs whole {}",
                i,
                c,
                w
            );
        }
    }

    /// A deterministic non-trivial test signal (two mixed tones).
    fn test_signal(len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| (i as f32 * 0.013).sin() + 0.3 * (i as f32 * 0.007).cos())
            .collect()
    }

    #[test]
    fn test_same_rate() {
        let input = vec![1.0, 2.0, 3.0];
        let output = resample(&input, 16_000, 16_000);
        assert_eq!(output, input);
    }

    #[test]
    fn test_downsample_to_16k() {
        // 48kHz -> 16kHz should produce roughly 1/3 the samples
        let input: Vec<f32> = (0..48000).map(|i| (i as f32 / 48000.0).sin()).collect();
        let output = resample(&input, 48_000, 16_000);
        assert!((output.len() as f64 - 16000.0).abs() < 2.0);
    }

    #[test]
    fn streaming_matches_whole_clip_downsample_48k() {
        let input = test_signal(48_000);
        let whole = resample(&input, 48_000, 16_000);
        // Uniform ~300 ms capture-callback-sized chunks.
        let chunked = resample_chunked(&input, 48_000, 16_000, &[14_400]);
        assert_chunked_matches_whole(&chunked, &whole, 2);
    }

    #[test]
    fn streaming_matches_whole_clip_non_integer_ratio() {
        // 44.1 kHz -> 16 kHz has a fractional ratio, so the seam position
        // lands mid-sample almost every chunk — the case the carried
        // fractional position exists for.
        let input = test_signal(44_100);
        let whole = resample(&input, 44_100, 16_000);
        let chunked = resample_chunked(&input, 44_100, 16_000, &[4_410]);
        assert_chunked_matches_whole(&chunked, &whole, 2);
    }

    #[test]
    fn streaming_matches_whole_clip_irregular_chunks() {
        // Pathological chunking (down to single samples) must still be
        // seam-exact: no duplicated or skipped output at any boundary.
        let input = test_signal(10_000);
        let whole = resample(&input, 48_000, 16_000);
        let chunked = resample_chunked(&input, 48_000, 16_000, &[1, 7, 480, 3, 1_000, 2]);
        assert_chunked_matches_whole(&chunked, &whole, 2);
    }

    #[test]
    fn streaming_matches_whole_clip_upsample() {
        let input = test_signal(4_000);
        let whole = resample(&input, 16_000, 48_000);
        let chunked = resample_chunked(&input, 16_000, 48_000, &[160, 3, 555]);
        // Upsampling withholds up to ceil(to/from) samples at the stream
        // head of the final source sample.
        assert_chunked_matches_whole(&chunked, &whole, 4);
    }

    #[test]
    fn streaming_single_chunk_matches_whole_clip() {
        let input = test_signal(9_600);
        let whole = resample(&input, 48_000, 16_000);
        let mut r = StreamingResampler::new(48_000, 16_000);
        let chunked = r.process(&input);
        assert_chunked_matches_whole(&chunked, &whole, 2);
    }

    #[test]
    fn streaming_same_rate_is_passthrough() {
        let input = test_signal(1_000);
        let mut r = StreamingResampler::new(16_000, 16_000);
        let mut out = r.process(&input[..300]);
        out.extend(r.process(&input[300..]));
        assert_eq!(out, input);
    }

    #[test]
    fn streaming_empty_chunk_is_noop() {
        let mut r = StreamingResampler::new(48_000, 16_000);
        assert!(r.process(&[]).is_empty());
        let out = r.process(&test_signal(96));
        assert!(!out.is_empty());
    }
}
