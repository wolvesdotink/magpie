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

#[cfg(test)]
mod tests {
    use super::*;

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
}
