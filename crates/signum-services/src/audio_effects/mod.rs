//! Audio effects chain and built-in effects

mod native;
pub mod native_instruments;
pub mod vst3;

pub use native::{
    CompressorEffect, DelayEffect, GainEffect, HighPassEffect, LowPassEffect, ReverbEffect,
};
pub use native_instruments::{Drum808, SampleKit, Sampler};
pub use vst3::{
    NativeWindowHandle, PluginGuiManager, PluginGuiWindow, Vst3Effect, Vst3Error,
    Vst3GuiError, Vst3Instrument, Vst3PluginInfo, Vst3Scanner,
};

// Re-export drum MIDI constants for UI
pub use native_instruments::drum808::{
    KICK, RIM_SHOT, SNARE, CLAP, CLOSED_HAT, OPEN_HAT, LOW_TOM, MID_TOM, HIGH_TOM,
    CRASH, COWBELL, HI_CONGA, MID_CONGA, LOW_CONGA, MARACAS, CLAVES,
};

use std::fmt::Debug;

/// Audio effect that can process samples in-place
pub trait AudioEffect: Send + Debug {
    fn name(&self) -> &str;
    fn process(&mut self, samples: &mut [f32]);
    fn set_param(&mut self, name: &str, value: f32);
    fn get_params(&self) -> Vec<EffectParam>;
    fn set_bypass(&mut self, bypass: bool);
    fn is_bypassed(&self) -> bool;
    /// Update sample rate for effects that depend on it
    fn set_sample_rate(&mut self, _sample_rate: f32) {}
}

/// Audio instrument that generates sound from MIDI input
pub trait AudioInstrument: Send {
    /// Instrument display name
    fn name(&self) -> &str;
    /// Queue a note-on event at the given sample offset
    fn queue_note_on(&mut self, pitch: u8, velocity: u8, channel: u8, sample_offset: u32);
    /// Queue a note-off event at the given sample offset
    fn queue_note_off(&mut self, pitch: u8, velocity: u8, channel: u8, sample_offset: u32);
    /// Turn off all notes immediately
    fn all_notes_off(&mut self);
    /// Process and return stereo output buffers
    fn process(&mut self, num_frames: usize) -> (&[f32], &[f32]);
    /// Update sample rate
    fn set_sample_rate(&mut self, sample_rate: f32);
    /// Get all parameters
    fn get_params(&self) -> &[EffectParam];
    /// Set parameter by name
    fn set_param(&mut self, name: &str, value: f32);
    /// Set parameter by index
    fn set_param_by_index(&mut self, index: usize, value: f64);
    /// Whether this is a drum instrument (for UI to show drum roll vs piano roll)
    fn is_drum(&self) -> bool { false }
}

/// Unified instrument wrapper for VST3 and native instruments
pub enum Instrument {
    Vst3(Vst3Instrument),
    Drum808(Drum808),
    Sampler(Sampler),
    SampleKit(SampleKit),
}

impl Instrument {
    pub fn name(&self) -> &str {
        match self {
            Self::Vst3(v) => v.name(),
            Self::Drum808(d) => d.name(),
            Self::Sampler(s) => s.name(),
            Self::SampleKit(k) => k.name(),
        }
    }

    pub fn queue_note_on(&mut self, pitch: u8, velocity: u8, channel: u8, sample_offset: u32) {
        match self {
            Self::Vst3(v) => v.queue_note_on(pitch, velocity, channel, sample_offset),
            Self::Drum808(d) => d.queue_note_on(pitch, velocity, channel, sample_offset),
            Self::Sampler(s) => s.queue_note_on(pitch, velocity, channel, sample_offset),
            Self::SampleKit(k) => k.queue_note_on(pitch, velocity, channel, sample_offset),
        }
    }

    pub fn queue_note_off(&mut self, pitch: u8, velocity: u8, channel: u8, sample_offset: u32) {
        match self {
            Self::Vst3(v) => v.queue_note_off(pitch, velocity, channel, sample_offset),
            Self::Drum808(d) => d.queue_note_off(pitch, velocity, channel, sample_offset),
            Self::Sampler(s) => s.queue_note_off(pitch, velocity, channel, sample_offset),
            Self::SampleKit(k) => k.queue_note_off(pitch, velocity, channel, sample_offset),
        }
    }

    pub fn all_notes_off(&mut self, sample_offset: u32) {
        match self {
            Self::Vst3(v) => v.all_notes_off(sample_offset),
            Self::Drum808(d) => { let _ = sample_offset; d.all_notes_off(); }
            Self::Sampler(s) => { let _ = sample_offset; s.all_notes_off(); }
            Self::SampleKit(k) => { let _ = sample_offset; k.all_notes_off(); }
        }
    }

    pub fn process(&mut self, num_frames: usize) -> (&[f32], &[f32]) {
        match self {
            Self::Vst3(v) => v.process(num_frames),
            Self::Drum808(d) => d.process(num_frames),
            Self::Sampler(s) => s.process(num_frames),
            Self::SampleKit(k) => k.process(num_frames),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        match self {
            Self::Vst3(v) => v.set_sample_rate(sample_rate),
            Self::Drum808(d) => d.set_sample_rate(sample_rate),
            Self::Sampler(s) => s.set_sample_rate(sample_rate),
            Self::SampleKit(k) => k.set_sample_rate(sample_rate),
        }
    }

    pub fn get_params(&self) -> &[EffectParam] {
        match self {
            Self::Vst3(v) => v.get_params(),
            Self::Drum808(d) => d.get_params(),
            Self::Sampler(s) => s.get_params(),
            Self::SampleKit(k) => k.get_params(),
        }
    }

    pub fn set_param(&mut self, name: &str, value: f32) {
        match self {
            Self::Vst3(v) => v.set_param(name, value),
            Self::Drum808(d) => d.set_param(name, value),
            Self::Sampler(s) => s.set_param(name, value),
            Self::SampleKit(k) => k.set_param(name, value),
        }
    }

    pub fn set_param_by_index(&mut self, index: usize, value: f64) {
        match self {
            Self::Vst3(v) => v.set_param_by_index(index, value),
            Self::Drum808(d) => d.set_param_by_index(index, value),
            Self::Sampler(s) => s.set_param_by_index(index, value),
            Self::SampleKit(k) => k.set_param_by_index(index, value),
        }
    }

    pub fn is_drum(&self) -> bool {
        match self {
            Self::Vst3(_) => false,
            Self::Drum808(_) => true,
            Self::Sampler(_) => false,
            Self::SampleKit(_) => true,
        }
    }

    /// Get VST3-specific plugin info (only for VST3 instruments)
    pub fn vst3_plugin_info(&self) -> Option<&Vst3PluginInfo> {
        match self {
            Self::Vst3(v) => Some(v.plugin_info()),
            Self::Drum808(_) | Self::Sampler(_) | Self::SampleKit(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EffectParam {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub unit: String,
}

impl EffectParam {
    pub fn new(name: &str, value: f32, min: f32, max: f32, unit: &str) -> Self {
        Self {
            name: name.to_string(),
            value,
            min,
            max,
            unit: unit.to_string(),
        }
    }
}

/// Chain of audio effects processed in order
#[derive(Debug, Default)]
pub struct EffectChain {
    effects: Vec<Box<dyn AudioEffect>>,
    bypass_all: bool,
}

impl EffectChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, effect: Box<dyn AudioEffect>) {
        self.effects.push(effect);
    }

    pub fn remove(&mut self, index: usize) -> Option<Box<dyn AudioEffect>> {
        if index >= self.effects.len() {
            return None;
        }
        Some(self.effects.remove(index))
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        if self.bypass_all {
            return;
        }
        for effect in &mut self.effects {
            if !effect.is_bypassed() {
                effect.process(samples);
            }
        }
    }

    pub fn set_bypass_all(&mut self, bypass: bool) {
        self.bypass_all = bypass;
    }

    pub fn is_bypass_all(&self) -> bool {
        self.bypass_all
    }

    pub fn effects(&self) -> &[Box<dyn AudioEffect>] {
        &self.effects
    }

    pub fn effects_mut(&mut self) -> &mut [Box<dyn AudioEffect>] {
        &mut self.effects
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn clear(&mut self) {
        self.effects.clear();
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        for effect in &mut self.effects {
            effect.set_sample_rate(sample_rate);
        }
    }
}
