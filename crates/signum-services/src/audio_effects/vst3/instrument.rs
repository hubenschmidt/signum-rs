//! VST3 instrument wrapper for MIDI-driven audio generation

use std::collections::{HashMap, HashSet};
use std::fmt;

use rack::{midi::MidiEvent, Plugin, PluginInstance, PluginScanner, Scanner};
use tracing::info;

use super::error::Vst3Error;
use super::scanner::Vst3PluginInfo;
use crate::audio_effects::{AudioInstrument, EffectParam};

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
    // Track active notes (pitches with Note On but no Note Off yet)
    active_notes: HashSet<u8>,
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
        let param_count = instance.parameter_count();
        let param_data: Vec<_> = (0..param_count)
            .filter_map(|i| {
                let pinfo = instance.parameter_info(i).ok()?;
                let current_value = instance.get_parameter(i).unwrap_or(pinfo.default);
                Some((i, pinfo, current_value))
            })
            .collect();

        let param_map: HashMap<_, _> = param_data
            .iter()
            .map(|(i, pinfo, _)| (pinfo.name.clone(), *i))
            .collect();

        let param_cache: Vec<_> = param_data
            .into_iter()
            .map(|(_, pinfo, value)| EffectParam::new(&pinfo.name, value, pinfo.min, pinfo.max, &pinfo.unit))
            .collect();

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
            active_notes: HashSet::new(),
        })
    }

    /// Get plugin info
    pub fn plugin_info(&self) -> &Vst3PluginInfo {
        &self.info
    }

    /// Get all parameters
    pub fn get_params(&self) -> &[EffectParam] {
        &self.param_cache
    }

    /// Set a parameter by name
    pub fn set_param(&mut self, name: &str, value: f32) {
        let Some(&index) = self.param_map.get(name) else { return };

        if let Err(e) = self.instance.set_parameter(index, value) {
            tracing::warn!("Failed to set parameter {}: {:?}", name, e);
            return;
        }

        // Update cache
        let Some(param) = self.param_cache.get_mut(index) else { return };
        param.value = value;
    }

    /// Set a parameter by index (normalized 0-1)
    /// The rack crate expects normalized values directly
    pub fn set_param_by_index(&mut self, index: usize, normalized_value: f64) {
        let normalized = normalized_value as f32;

        tracing::info!(
            "set_param_by_index: name={} index={} value={} total_params={}",
            self.info.name, index, normalized, self.instance.parameter_count()
        );

        if let Err(e) = self.instance.set_parameter(index, normalized) {
            tracing::warn!("Failed to set parameter index {}: {:?}", index, e);
            return;
        }

        tracing::info!("Successfully set parameter {} = {}", index, normalized);

        // Update cache with denormalized value for display
        let Some(param) = self.param_cache.get_mut(index) else { return };
        param.value = param.min + normalized * (param.max - param.min);
    }

    /// Queue a note on event
    pub fn queue_note_on(&mut self, pitch: u8, velocity: u8, channel: u8, sample_offset: u32) {
        self.active_notes.insert(pitch);
        self.pending_events.push(MidiEvent::note_on(pitch, velocity, channel, sample_offset));
    }

    /// Queue a note off event
    pub fn queue_note_off(&mut self, pitch: u8, velocity: u8, channel: u8, sample_offset: u32) {
        self.active_notes.remove(&pitch);
        self.pending_events.push(MidiEvent::note_off(pitch, velocity, channel, sample_offset));
    }

    /// Send note off for all currently active notes (used when loop wraps to stop hanging notes)
    pub fn all_notes_off(&mut self, sample_offset: u32) {
        for pitch in self.active_notes.drain() {
            self.pending_events.push(MidiEvent::note_off(pitch, 0, 0, sample_offset));
        }
    }

    /// Process pending MIDI events and generate audio
    /// Returns stereo output buffers (left, right)
    pub fn process(&mut self, num_frames: usize) -> (&[f32], &[f32]) {
        let frames = num_frames.min(self.max_block_size);

        // Send queued MIDI to plugin
        self.flush_pending_midi();

        // Clear output buffers
        self.output_left[..frames].fill(0.0);
        self.output_right[..frames].fill(0.0);

        // Process with silent input buffers (instruments generate audio from MIDI)
        let inputs: [&[f32]; 2] = [&self.input_left[..frames], &self.input_right[..frames]];
        let mut outputs: [&mut [f32]; 2] = [
            &mut self.output_left[..frames],
            &mut self.output_right[..frames],
        ];

        if let Err(e) = self.instance.process(&inputs, &mut outputs, frames) {
            tracing::warn!("VST3 instrument process error: {:?}", e);
        }

        (&self.output_left[..frames], &self.output_right[..frames])
    }

    fn flush_pending_midi(&mut self) {
        if self.pending_events.is_empty() {
            return;
        }
        if let Err(e) = self.instance.send_midi(&self.pending_events) {
            tracing::warn!("Failed to send MIDI: {:?}", e);
        }
        self.pending_events.clear();
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

impl AudioInstrument for Vst3Instrument {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn queue_note_on(&mut self, pitch: u8, velocity: u8, channel: u8, sample_offset: u32) {
        Vst3Instrument::queue_note_on(self, pitch, velocity, channel, sample_offset);
    }

    fn queue_note_off(&mut self, pitch: u8, velocity: u8, channel: u8, sample_offset: u32) {
        Vst3Instrument::queue_note_off(self, pitch, velocity, channel, sample_offset);
    }

    fn all_notes_off(&mut self) {
        Vst3Instrument::all_notes_off(self, 0);
    }

    fn process(&mut self, num_frames: usize) -> (&[f32], &[f32]) {
        Vst3Instrument::process(self, num_frames)
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        Vst3Instrument::set_sample_rate(self, sample_rate);
    }

    fn get_params(&self) -> &[EffectParam] {
        Vst3Instrument::get_params(self)
    }

    fn set_param(&mut self, name: &str, value: f32) {
        Vst3Instrument::set_param(self, name, value);
    }

    fn set_param_by_index(&mut self, index: usize, value: f64) {
        Vst3Instrument::set_param_by_index(self, index, value);
    }

    fn is_drum(&self) -> bool {
        false
    }
}
