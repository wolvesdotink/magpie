use crate::constants::WHISPER_SAMPLE_RATE;

/// Resample audio from `from_rate` to 16kHz using linear interpolation.
/// This is a simple approach that works well enough for speech recognition.
pub fn resample_to_16khz(samples: &[f32], from_rate: u32) -> Vec<f32> {
    let target_rate = WHISPER_SAMPLE_RATE;

    if from_rate == target_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / target_rate as f64;
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
        let output = resample_to_16khz(&input, 16_000);
        assert_eq!(output, input);
    }

    #[test]
    fn test_downsample() {
        // 48kHz -> 16kHz should produce roughly 1/3 the samples
        let input: Vec<f32> = (0..48000).map(|i| (i as f32 / 48000.0).sin()).collect();
        let output = resample_to_16khz(&input, 48_000);
        assert!((output.len() as f64 - 16000.0).abs() < 2.0);
    }
}
