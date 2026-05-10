use std::sync::Arc;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};

use crate::state::AppState;

/// Start recording from the default input device.
/// Returns the cpal Stream handle (drop it to stop recording) and the device sample rate.
pub fn start_recording(state: &Arc<AppState>) -> Result<(Stream, u32)> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("No input device available")?;

    log::info!("Using input device: {}", device.name().unwrap_or_default());

    let config = device
        .default_input_config()
        .context("Failed to get default input config")?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    log::info!(
        "Recording at {} Hz, {} channels, format: {:?}",
        sample_rate,
        channels,
        config.sample_format()
    );

    // Clear existing buffer
    {
        let mut buffer = state.audio_buffer.lock().unwrap();
        buffer.clear();
    }

    // Store the sample rate for resampling later
    {
        let mut sr = state.capture_sample_rate.lock().unwrap();
        *sr = sample_rate;
    }

    let state_clone = Arc::clone(state);
    let stream_config: StreamConfig = config.clone().into();

    let err_fn = |err: cpal::StreamError| {
        log::error!("Audio stream error: {}", err);
    };

    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mono = to_mono(data, channels);
                // Update amplitude for visualization (lock-free)
                let sum_sq: f32 = mono.iter().map(|&s| s * s).sum();
                let rms = (sum_sq / mono.len().max(1) as f32).sqrt();
                state_clone.set_amplitude(rms);
                let mut buffer = state_clone.audio_buffer.lock().unwrap();
                buffer.extend_from_slice(&mono);
            },
            err_fn,
            None,
        )?,
        SampleFormat::I16 => {
            let state_clone2 = Arc::clone(state);
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let float_data: Vec<f32> =
                        data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                    let mono = to_mono(&float_data, channels);
                    let sum_sq: f32 = mono.iter().map(|&s| s * s).sum();
                    let rms = (sum_sq / mono.len().max(1) as f32).sqrt();
                    state_clone2.set_amplitude(rms);
                    let mut buffer = state_clone2.audio_buffer.lock().unwrap();
                    buffer.extend_from_slice(&mono);
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::U16 => {
            let state_clone2 = Arc::clone(state);
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    let float_data: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                        .collect();
                    let mono = to_mono(&float_data, channels);
                    let sum_sq: f32 = mono.iter().map(|&s| s * s).sum();
                    let rms = (sum_sq / mono.len().max(1) as f32).sqrt();
                    state_clone2.set_amplitude(rms);
                    let mut buffer = state_clone2.audio_buffer.lock().unwrap();
                    buffer.extend_from_slice(&mono);
                },
                err_fn,
                None,
            )?
        }
        format => anyhow::bail!("Unsupported sample format: {:?}", format),
    };

    stream.play().context("Failed to start audio stream")?;

    Ok((stream, sample_rate))
}

/// Convert interleaved multi-channel audio to mono by averaging channels
fn to_mono(data: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return data.to_vec();
    }

    data.chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}
