//! Audio input service for microphone/line capture

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, SampleFormat, StreamConfig};
use crossbeam_channel::Sender;
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum AudioInputError {
    #[error("No audio input devices found")]
    NoDevices,
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    #[error("Failed to get input config: {0}")]
    ConfigError(String),
    #[error("Failed to build input stream: {0}")]
    StreamError(String),
}

/// Audio input device info
#[derive(Debug, Clone)]
pub struct InputDevice {
    pub id: String,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub is_default: bool,
}

/// Handle to stop a running input stream
pub struct InputStreamHandle {
    stop_flag: Arc<AtomicBool>,
    _stream: cpal::Stream,
}

impl InputStreamHandle {
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

impl Drop for InputStreamHandle {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

pub struct AudioInputService;

impl AudioInputService {
    /// List available input devices
    pub fn list_devices() -> Result<Vec<InputDevice>, AudioInputError> {
        let host = cpal::default_host();
        let default_device = host.default_input_device();
        let default_name = default_device.as_ref().and_then(|d| d.name().ok());

        let devices: Vec<_> = host
            .input_devices()
            .map_err(|e| AudioInputError::ConfigError(e.to_string()))?
            .filter_map(|device| {
                let name = device.name().ok()?;
                let config = device.default_input_config().ok()?;

                Some(InputDevice {
                    id: name.clone(),
                    name: name.clone(),
                    sample_rate: config.sample_rate().0,
                    channels: config.channels(),
                    is_default: default_name.as_ref() == Some(&name),
                })
            })
            .collect();

        if devices.is_empty() {
            return Err(AudioInputError::NoDevices);
        }

        info!(count = devices.len(), "Found audio input devices");
        Ok(devices)
    }

    /// Get device by ID (or default if "default")
    fn get_device(device_id: &str) -> Result<Device, AudioInputError> {
        let host = cpal::default_host();

        if device_id == "default" {
            return host
                .default_input_device()
                .ok_or(AudioInputError::NoDevices);
        }

        for device in host
            .input_devices()
            .map_err(|e| AudioInputError::ConfigError(e.to_string()))?
        {
            if let Ok(name) = device.name() {
                if name == device_id {
                    return Ok(device);
                }
            }
        }

        Err(AudioInputError::DeviceNotFound(device_id.to_string()))
    }

    /// Get default input device info
    pub fn get_default_device_info() -> Result<(String, u32, u16), AudioInputError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(AudioInputError::NoDevices)?;

        let config = device
            .default_input_config()
            .map_err(|e| AudioInputError::ConfigError(e.to_string()))?;

        let name = device.name().unwrap_or_default();
        Ok((name, config.sample_rate().0, config.channels()))
    }

    /// Start streaming audio input to a channel
    /// Returns (handle, sample_rate, channels)
    pub fn start_stream(
        device_id: &str,
        chunk_tx: Sender<Vec<f32>>,
    ) -> Result<(InputStreamHandle, u32, u16), AudioInputError> {
        let device = Self::get_device(device_id)?;
        let config = device
            .default_input_config()
            .map_err(|e| AudioInputError::ConfigError(e.to_string()))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();

        info!(
            device = %device.name().unwrap_or_default(),
            sample_rate,
            channels,
            "Starting audio input stream"
        );

        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_flag.clone();

        let stream_config: StreamConfig = config.clone().into();

        let stream = match config.sample_format() {
            SampleFormat::F32 => Self::build_stream::<f32>(&device, &stream_config, chunk_tx, stop_clone),
            SampleFormat::I16 => Self::build_stream::<i16>(&device, &stream_config, chunk_tx, stop_clone),
            SampleFormat::I32 => Self::build_stream::<i32>(&device, &stream_config, chunk_tx, stop_clone),
            format => return Err(AudioInputError::ConfigError(format!("Unsupported format: {:?}", format))),
        }?;

        stream.play().map_err(|e| AudioInputError::StreamError(e.to_string()))?;

        Ok((InputStreamHandle { stop_flag, _stream: stream }, sample_rate, channels))
    }

    fn build_stream<T>(
        device: &Device,
        config: &StreamConfig,
        tx: Sender<Vec<f32>>,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<cpal::Stream, AudioInputError>
    where
        T: cpal::Sample + cpal::SizedSample + Send + 'static,
        f32: cpal::FromSample<T>,
    {
        device
            .build_input_stream(
                config,
                move |data: &[T], _: &cpal::InputCallbackInfo| {
                    if stop_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    let samples: Vec<f32> = data.iter().map(|s| f32::from_sample_(*s)).collect();
                    let _ = tx.try_send(samples);
                },
                |err| error!("Input stream error: {}", err),
                None,
            )
            .map_err(|e| AudioInputError::StreamError(e.to_string()))
    }
}
