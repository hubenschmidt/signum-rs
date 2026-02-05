//! Input monitoring with VU metering and recording

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crossbeam_channel::{bounded, Receiver, Sender};
use thiserror::Error;
use tracing::info;

use crate::audio_input::{AudioInputError, AudioInputService, InputStreamHandle};
use crate::audio_io::{AudioOutputError, RealtimeOutputStream};
use crate::audio_effects::EffectChain;

#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("Input error: {0}")]
    Input(#[from] AudioInputError),
    #[error("Output error: {0}")]
    Output(#[from] AudioOutputError),
    #[error("Monitor already running")]
    AlreadyRunning,
    #[error("Monitor not running")]
    NotRunning,
    #[error("Not recording")]
    NotRecording,
}

/// Shared metering state (lock-free reads from UI)
pub struct MeterState {
    peak_raw: AtomicU32,
    rms_raw: AtomicU32,
    clipped: AtomicBool,
}

impl MeterState {
    fn new() -> Self {
        Self {
            peak_raw: AtomicU32::new(0),
            rms_raw: AtomicU32::new(0),
            clipped: AtomicBool::new(false),
        }
    }

    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak_raw.load(Ordering::Relaxed))
    }

    pub fn rms(&self) -> f32 {
        f32::from_bits(self.rms_raw.load(Ordering::Relaxed))
    }

    pub fn is_clipped(&self) -> bool {
        self.clipped.load(Ordering::Relaxed)
    }

    pub fn clear_clip(&self) {
        self.clipped.store(false, Ordering::Relaxed);
    }

    fn set_peak(&self, val: f32) {
        self.peak_raw.store(val.to_bits(), Ordering::Relaxed);
    }

    fn set_rms(&self, val: f32) {
        self.rms_raw.store(val.to_bits(), Ordering::Relaxed);
    }

    fn set_clipped(&self) {
        self.clipped.store(true, Ordering::Relaxed);
    }
}

impl Default for MeterState {
    fn default() -> Self {
        Self::new()
    }
}

/// Recorded audio data
pub struct RecordedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Input monitor with pass-through, metering, and recording
pub struct InputMonitor {
    meter_state: Arc<MeterState>,
    input_handle: Option<InputStreamHandle>,
    output_stream: Option<RealtimeOutputStream>,
    monitor_enabled: Arc<AtomicBool>,
    recording: Arc<AtomicBool>,
    record_buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
    effects: Arc<Mutex<EffectChain>>,
}

impl InputMonitor {
    pub fn new() -> Self {
        Self {
            meter_state: Arc::new(MeterState::new()),
            input_handle: None,
            output_stream: None,
            monitor_enabled: Arc::new(AtomicBool::new(false)),
            recording: Arc::new(AtomicBool::new(false)),
            record_buffer: Arc::new(Mutex::new(Vec::new())),
            sample_rate: 44100,
            channels: 2,
            effects: Arc::new(Mutex::new(EffectChain::new())),
        }
    }

    pub fn meter_state(&self) -> Arc<MeterState> {
        self.meter_state.clone()
    }

    pub fn is_running(&self) -> bool {
        self.input_handle.is_some()
    }

    pub fn is_monitor_enabled(&self) -> bool {
        self.monitor_enabled.load(Ordering::SeqCst)
    }

    pub fn set_monitor_enabled(&self, enabled: bool) {
        self.monitor_enabled.store(enabled, Ordering::SeqCst);
        info!(enabled, "Monitor pass-through toggled");
    }

    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn with_effects<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut EffectChain) -> R,
    {
        self.effects.lock().ok().map(|mut e| f(&mut e))
    }

    /// Start recording (monitor must be running)
    pub fn start_recording(&self) -> Result<(), MonitorError> {
        if !self.is_running() {
            return Err(MonitorError::NotRunning);
        }

        // Clear buffer
        if let Ok(mut buf) = self.record_buffer.lock() {
            buf.clear();
        }

        self.recording.store(true, Ordering::SeqCst);
        info!("Recording started");
        Ok(())
    }

    /// Stop recording and return recorded audio
    pub fn stop_recording(&self) -> Result<RecordedAudio, MonitorError> {
        if !self.is_recording() {
            return Err(MonitorError::NotRecording);
        }

        self.recording.store(false, Ordering::SeqCst);

        let samples = self.record_buffer
            .lock()
            .map(|mut buf| std::mem::take(&mut *buf))
            .unwrap_or_default();

        info!(samples = samples.len(), "Recording stopped");

        Ok(RecordedAudio {
            samples,
            sample_rate: self.sample_rate,
            channels: 1, // We record mono
        })
    }

    /// Get current recording buffer for live preview (doesn't stop recording)
    pub fn get_recording_preview(&self) -> Option<Vec<f32>> {
        if !self.is_recording() {
            return None;
        }
        self.record_buffer.lock().ok().map(|buf| buf.to_vec())
    }

    /// Get current recording length in samples
    pub fn recording_length(&self) -> usize {
        self.record_buffer
            .lock()
            .map(|buf| buf.len())
            .unwrap_or(0)
    }

    /// Start input monitoring
    pub fn start(&mut self, device_id: &str) -> Result<(), MonitorError> {
        if self.input_handle.is_some() {
            return Err(MonitorError::AlreadyRunning);
        }

        let (audio_tx, audio_rx) = bounded::<Vec<f32>>(64);

        let (input_handle, sample_rate, channels) = AudioInputService::start_stream(device_id, audio_tx)?;

        self.sample_rate = sample_rate;
        self.channels = channels;
        self.input_handle = Some(input_handle);

        if let Ok(mut fx) = self.effects.lock() {
            fx.set_sample_rate(sample_rate as f32);
        }

        let meter_state = self.meter_state.clone();
        let monitor_enabled = self.monitor_enabled.clone();
        let recording = self.recording.clone();
        let record_buffer = self.record_buffer.clone();
        let effects = self.effects.clone();
        let (out_tx, out_rx) = bounded::<Vec<f32>>(64);

        thread::spawn(move || {
            Self::process_loop(
                audio_rx,
                out_tx,
                meter_state,
                monitor_enabled,
                recording,
                record_buffer,
                effects,
                channels as usize,
            );
        });

        let output_stream = RealtimeOutputStream::start(move |buffer, _sr, out_channels| {
            let out_channels = out_channels as usize;

            match out_rx.try_recv() {
                Ok(samples) => {
                    let mut idx = 0;
                    for frame in buffer.chunks_mut(out_channels) {
                        let mono = samples.get(idx).copied().unwrap_or(0.0);
                        for ch in frame.iter_mut() {
                            *ch = mono;
                        }
                        idx += 1;
                    }
                }
                Err(_) => buffer.fill(0.0),
            }
        })?;

        self.output_stream = Some(output_stream);

        info!(device = device_id, sample_rate, channels, "Input monitor started");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), MonitorError> {
        // Stop recording first if active
        if self.is_recording() {
            self.recording.store(false, Ordering::SeqCst);
        }

        let input = self.input_handle.take().ok_or(MonitorError::NotRunning)?;
        input.stop();

        if let Some(output) = self.output_stream.take() {
            output.stop();
        }

        self.meter_state.set_peak(0.0);
        self.meter_state.set_rms(0.0);

        info!("Input monitor stopped");
        Ok(())
    }

    fn process_loop(
        rx: Receiver<Vec<f32>>,
        tx: Sender<Vec<f32>>,
        meter: Arc<MeterState>,
        monitor_enabled: Arc<AtomicBool>,
        recording: Arc<AtomicBool>,
        record_buffer: Arc<Mutex<Vec<f32>>>,
        effects: Arc<Mutex<EffectChain>>,
        channels: usize,
    ) {
        let mut peak_hold = 0.0f32;
        let peak_decay = 0.95f32;

        while let Ok(samples) = rx.recv() {
            // Convert to mono
            let mono: Vec<f32> = samples
                .chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect();

            // Calculate peak
            let current_peak = mono.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            peak_hold = f32::max(current_peak, peak_hold * peak_decay);

            // Calculate RMS
            let rms = (mono.iter().map(|s| s * s).sum::<f32>() / mono.len() as f32).sqrt();

            // Update meter state
            meter.set_peak(peak_hold);
            meter.set_rms(rms);

            if current_peak > 1.0 {
                meter.set_clipped();
            }

            // Record if enabled
            if recording.load(Ordering::SeqCst) {
                if let Ok(mut buf) = record_buffer.lock() {
                    buf.extend(&mono);
                }
            }

            // Pass-through if enabled
            if !monitor_enabled.load(Ordering::SeqCst) {
                continue;
            }

            let mut processed = mono;

            if let Ok(mut fx) = effects.lock() {
                fx.process(&mut processed);
            }

            let _ = tx.try_send(processed);
        }
    }
}

impl Default for InputMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for InputMonitor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
