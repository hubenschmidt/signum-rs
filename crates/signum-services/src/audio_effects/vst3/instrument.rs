//! VST3 instrument wrapper for MIDI-driven audio generation

use std::collections::HashMap;
use std::fmt;

use rack::{midi::MidiEvent, Plugin, PluginInstance, PluginScanner, Scanner};
use tracing::info;

use super::error::Vst3Error;
use super::scanner::Vst3PluginInfo;
use crate::audio_effects::EffectParam;

/// VST3 instrument that generates audio from MIDI input
pub struct Vst3Instrument {
    instance: Plugin,
    info: Vst3PluginInfo,
    sample_rate: f32,
    // Pre-allocated input buffers (silent, for instruments that require input)
    input_left: Vec<f32>,
    input_right: Vec<f32>,
    // Pre-allocated output buffers
    output_left: Vec<f32>,
    output_right: Vec<f32>,
    max_block_size: usize,
    // Pending MIDI events for next process call
    pending_events: Vec<MidiEvent>,
    // Parameter name -> index mapping
    param_map: HashMap<String, usize>,
    // Cached parameter info
    param_cache: Vec<EffectParam>,
}

// Safety: We ensure single-threaded access via Mutex
unsafe impl Send for Vst3Instrument {}

impl Vst3Instrument {
    /// Create a new VST3 instrument from plugin info
    pub fn new(
        scanner: &Scanner,
        info: &Vst3PluginInfo,
        sample_rate: f32,
    ) -> Result<Self, Vst3Error> {
        let mut instance = scanner
            .load(&info.info)
            .map_err(|e| Vst3Error::LoadError(format!("{:?}", e)))?;

        let max_block_size = 4096;
        instance
            .initialize(sample_rate as f64, max_block_size)
            .map_err(|e| Vst3Error::LoadError(format!("{:?}", e)))?;

        // Build parameter map
        let mut param_map = HashMap::new();
        let mut param_cache = Vec::new();
        let param_count = instance.parameter_count();

        for i in 0..param_count {
            if let Ok(pinfo) = instance.parameter_info(i) {
                param_map.insert(pinfo.name.clone(), i);
                let current_value = instance.get_parameter(i).unwrap_or(pinfo.default);
                param_cache.push(EffectParam::new(
                    &pinfo.name,
                    current_value,
                    pinfo.min,
                    pinfo.max,
                    &pinfo.unit,
                ));
            }
        }

        info!(name = %info.name, sample_rate, params = param_count, "VST3 instrument loaded");

        Ok(Self {
            instance,
            info: info.clone(),
            sample_rate,
            input_left: vec![0.0; max_block_size],
            input_right: vec![0.0; max_block_size],
            output_left: vec![0.0; max_block_size],
            output_right: vec![0.0; max_block_size],
            max_block_size,
            pending_events: Vec::with_capacity(256),
            param_map,
            param_cache,
        })
    }

    /// Get plugin info
    pub fn plugin_info(&self) -> &Vst3PluginInfo {
        &self.info
    }

    /// Get all parameters
    pub fn get_params(&self) -> Vec<EffectParam> {
        self.param_cache.clone()
    }

    /// Set a parameter by name
    pub fn set_param(&mut self, name: &str, value: f32) {
        let Some(&index) = self.param_map.get(name) else {
            return;
        };

        if let Err(e) = self.instance.set_parameter(index, value) {
            tracing::warn!("Failed to set parameter {}: {:?}", name, e);
            return;
        }

        // Update cache
        if let Some(param) = self.param_cache.get_mut(index) {
            param.value = value;
        }
    }

    /// Set a parameter by index (normalized 0-1)
    /// The rack crate expects normalized values directly
    pub fn set_param_by_index(&mut self, index: usize, normalized_value: f64) {
        let normalized = normalized_value as f32;
        let total_params = self.instance.parameter_count();

        tracing::info!(
            "set_param_by_index: name={} index={} value={} total_params={}",
            self.info.name, index, normalized, total_params
        );

        if let Err(e) = self.instance.set_parameter(index, normalized) {
            tracing::warn!("Failed to set parameter index {}: {:?}", index, e);
            return;
        }

        tracing::info!("Successfully set parameter {} = {}", index, normalized);

        // Update cache with denormalized value for display
        if let Some(param) = self.param_cache.get_mut(index) {
            param.value = param.min + normalized * (param.max - param.min);
        }
    }

    /// Set the component state (for preset/patch sync)
    pub fn set_state(&mut self, data: &[u8]) -> Result<(), Vst3Error> {
        self.instance
            .set_state(data)
            .map_err(|e| Vst3Error::LoadError(format!("Failed to set state: {:?}", e)))?;

        // Refresh parameter cache after state change
        let param_count = self.instance.parameter_count();
        for i in 0..param_count {
            if let Ok(value) = self.instance.get_parameter(i) {
                if let Some(param) = self.param_cache.get_mut(i) {
                    // Convert normalized to display value
                    param.value = param.min + value * (param.max - param.min);
                }
            }
        }

        Ok(())
    }

    /// Queue a note on event
    pub fn queue_note_on(&mut self, pitch: u8, velocity: u8, channel: u8, sample_offset: u32) {
        self.pending_events.push(MidiEvent::note_on(pitch, velocity, channel, sample_offset));
    }

    /// Queue a note off event
    pub fn queue_note_off(&mut self, pitch: u8, velocity: u8, channel: u8, sample_offset: u32) {
        self.pending_events.push(MidiEvent::note_off(pitch, velocity, channel, sample_offset));
    }

    /// Process pending MIDI events and generate audio
    /// Returns stereo output buffers (left, right)
    pub fn process(&mut self, num_frames: usize) -> (&[f32], &[f32]) {
        let frames = num_frames.min(self.max_block_size);

        // Send queued MIDI to plugin
        if !self.pending_events.is_empty() {
            if let Err(e) = self.instance.send_midi(&self.pending_events) {
                tracing::warn!("Failed to send MIDI: {:?}", e);
            }
            self.pending_events.clear();
        }

        // Clear output buffers
        self.output_left[..frames].fill(0.0);
        self.output_right[..frames].fill(0.0);

        // Process with silent input buffers (instruments generate audio from MIDI, not input)
        let inputs: [&[f32]; 2] = [
            &self.input_left[..frames],
            &self.input_right[..frames],
        ];
        let mut outputs: [&mut [f32]; 2] = [
            &mut self.output_left[..frames],
            &mut self.output_right[..frames],
        ];

        if let Err(e) = self.instance.process(&inputs, &mut outputs, frames) {
            tracing::warn!("VST3 instrument process error: {:?}", e);
        }

        (&self.output_left[..frames], &self.output_right[..frames])
    }

    /// Set sample rate (reinitializes plugin)
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if (sample_rate - self.sample_rate).abs() < 1.0 {
            return;
        }

        self.sample_rate = sample_rate;

        if let Err(e) = self.instance.initialize(sample_rate as f64, self.max_block_size) {
            tracing::error!("Failed to reinitialize VST3 instrument: {:?}", e);
        }
    }
}

impl fmt::Debug for Vst3Instrument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vst3Instrument")
            .field("name", &self.info.name)
            .field("pending_events", &self.pending_events.len())
            .finish()
    }
}
