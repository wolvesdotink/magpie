//! Conservative near-silence gate for preview work only. Audio is always kept
//! for the final pass; quiet speech is not removed from recordings.
#[derive(Default)]
pub struct PreviewGate {
    heard_audio: bool,
    quiet_samples: usize,
}

impl PreviewGate {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn should_decode(&mut self, samples: &[f32], rate: u32) -> bool {
        if rate == 0 {
            return false;
        }
        let mut active = false;
        // Inspect 20 ms frames, so a short utterance isn't diluted by a long
        // chunk received while an earlier decode was busy.
        for frame in samples.chunks((rate as usize / 50).max(1)) {
            let energy = frame.iter().map(|v| v * v).sum::<f32>() / frame.len() as f32;
            if energy.is_finite() && energy > 1e-8 {
                self.heard_audio = true;
                self.quiet_samples = 0;
                active = true;
            } else {
                self.quiet_samples = self.quiet_samples.saturating_add(frame.len());
            }
        }
        // Keep a 600 ms tail so the last word can settle when speech stops.
        active || (self.heard_audio && self.quiet_samples <= rate as usize * 3 / 5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_never_starts_decode_and_speech_has_a_tail() {
        let mut gate = PreviewGate::default();
        assert!(!gate.should_decode(&vec![0.0; 16000], 16000));
        assert!(gate.should_decode(&vec![0.001; 320], 16000));
        assert!(gate.should_decode(&vec![0.0; 4800], 16000));
        assert!(gate.should_decode(&vec![0.0; 4800], 16000));
        assert!(!gate.should_decode(&vec![0.0; 320], 16000));
        assert!(gate.should_decode(&vec![0.001; 320], 16000));
        gate.reset();
        assert!(!gate.should_decode(&vec![0.0; 320], 16000));
    }
    #[test]
    fn short_speech_in_a_backlogged_chunk_is_detected() {
        let mut samples = vec![0.0; 16000];
        samples[..320].fill(0.001);
        assert!(PreviewGate::default().should_decode(&samples, 16000));
    }
}
