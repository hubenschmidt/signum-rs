//! Audio output service for playback

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::StreamConfig;
use crossbeam_channel::{bounded, Receiver};
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum AudioOutputError {
    #[error("No audio output devices found")]
    NoDevices,
    #[error("Failed to get default output config: {0}")]
    ConfigError(String),
    #[error("Failed to build output stream: {0}")]
    StreamError(String),
    #[error("Playback failed: {0}")]
    PlaybackError(String),
}

/// Handle for async playback completion
pub struct PlaybackHandle {
    done_rx: Receiver<Result<(), AudioOutputError>>,
    _stream: cpal::Stream,
}

impl PlaybackHandle {
    pub fn wait(self) -> Result<(), AudioOutputError> {
        self.done_rx.recv().ok().unwrap_or(Ok(()))
    }

    pub fn is_done(&self) -> bool {
        !self.done_rx.is_empty()
    }
}

pub struct AudioOutputService;

impl AudioOutputService {
    /// Play f32 samples through default output device (blocking)
    pub fn play_samples_blocking(samples: &[f32], sample_rate: u32) -> Result<(), AudioOutputError> {
        let handle = Self::play_samples(samples.to_vec(), sample_rate)?;
        handle.wait()
    }

    /// Play f32 samples through default output device (async)
    pub fn play_samples(samples: Vec<f32>, sample_rate: u32) -> Result<PlaybackHandle, AudioOutputError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioOutputError::NoDevices)?;

        let supported_config = device
            .default_output_config()
            .map_err(|e| AudioOutputError::ConfigError(e.to_string()))?;

        let device_sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels() as usize;

        info!(
            device = %device.name().unwrap_or_default(),
            sample_rate = device_sample_rate,
            channels = channels,
            input_samples = samples.len(),
            "Starting audio playback"
        );

        // Resample if necessary
        let resampled = Self::resample_if_needed(&samples, sample_rate, device_sample_rate)?;

        // Normalize to prevent clipping
        let max_amp = resampled.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        let gain = if max_amp > 0.001 { 0.8 / max_amp } else { 1.0 };
        let normalized: Vec<f32> = resampled.iter().map(|s| s * gain).collect();

        // Convert mono to output channels
        let output_samples: Vec<f32> = normalized
            .iter()
            .flat_map(|&s| std::iter::repeat(s).take(channels))
            .collect();

        let samples_arc = Arc::new(output_samples);
        let position = Arc::new(AtomicUsize::new(0));
        let position_clone = position.clone();
        let samples_clone = samples_arc.clone();
        let total_samples = samples_arc.len();

        let (done_tx, done_rx) = bounded(1);

        let config: StreamConfig = supported_config.into();

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let pos = position_clone.load(Ordering::SeqCst);
                    for (i, sample) in data.iter_mut().enumerate() {
                        *sample = samples_clone.get(pos + i).copied().unwrap_or(0.0);
                    }
                    let new_pos = (pos + data.len()).min(total_samples);
                    position_clone.store(new_pos, Ordering::SeqCst);
                },
                move |err| error!("Playback stream error: {}", err),
                None,
            )
            .map_err(|e| AudioOutputError::StreamError(e.to_string()))?;

        stream.play().map_err(|e| AudioOutputError::StreamError(e.to_string()))?;

        // Monitor completion
        let position_monitor = position;
        thread::spawn(move || {
            let duration_secs = total_samples as f64 / (device_sample_rate as f64 * channels as f64);
            let timeout = Duration::from_secs_f64(duration_secs + 0.5);
            let start = std::time::Instant::now();

            while start.elapsed() < timeout {
                let pos = position_monitor.load(Ordering::SeqCst);
                if pos >= total_samples {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            let _ = done_tx.send(Ok(()));
        });

        Ok(PlaybackHandle { done_rx, _stream: stream })
    }

    fn resample_if_needed(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, AudioOutputError> {
        if from_rate == to_rate {
            return Ok(samples.to_vec());
        }

        use rubato::{SincFixedIn, SincInterpolationParameters, SincInterpolationType, Resampler, WindowFunction};

        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        let mut resampler = SincFixedIn::<f32>::new(
            to_rate as f64 / from_rate as f64,
            2.0,
            params,
            samples.len(),
            1,
        ).map_err(|e| AudioOutputError::PlaybackError(format!("Resample init error: {}", e)))?;

        let input = vec![samples.to_vec()];
        let output = resampler
            .process(&input, None)
            .map_err(|e| AudioOutputError::PlaybackError(format!("Resample error: {}", e)))?;

        Ok(output.into_iter().flatten().collect())
    }

    /// Get default output device info
    pub fn get_default_device_info() -> Result<(String, u32, u16), AudioOutputError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioOutputError::NoDevices)?;

        let config = device
            .default_output_config()
            .map_err(|e| AudioOutputError::ConfigError(e.to_string()))?;

        let name = device.name().unwrap_or_default();
        Ok((name, config.sample_rate().0, config.channels()))
    }
}

/// Real-time audio output stream for engine playback
pub struct RealtimeOutputStream {
    stop_flag: Arc<AtomicBool>,
    _stream: cpal::Stream,
}

impl RealtimeOutputStream {
    /// Start a real-time output stream that pulls samples from a callback
    pub fn start<F>(sample_callback: F) -> Result<Self, AudioOutputError>
    where
        F: FnMut(&mut [f32], u32, u16) + Send + 'static,
    {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioOutputError::NoDevices)?;

        let supported_config = device
            .default_output_config()
            .map_err(|e| AudioOutputError::ConfigError(e.to_string()))?;

        let sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels();

        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_flag.clone();

        let config: StreamConfig = supported_config.into();
        let callback = Arc::new(Mutex::new(sample_callback));

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if stop_clone.load(Ordering::SeqCst) {
                        data.fill(0.0);
                        return;
                    }
                    let Ok(mut cb) = callback.lock() else {
                        data.fill(0.0);
                        return;
                    };
                    cb(data, sample_rate, channels);
                },
                move |err| error!("Output stream error: {}", err),
                None,
            )
            .map_err(|e| AudioOutputError::StreamError(e.to_string()))?;

        stream.play().map_err(|e| AudioOutputError::StreamError(e.to_string()))?;

        info!(sample_rate, channels, "Started realtime output stream");

        Ok(Self { stop_flag, _stream: stream })
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

impl Drop for RealtimeOutputStream {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}
