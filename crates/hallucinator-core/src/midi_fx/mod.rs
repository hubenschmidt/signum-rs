//! MIDI effects for Factory Rat-style real-time processing

mod arpeggiator;
mod chance;
mod echo;
mod harmonizer;
mod humanize;
mod quantize;
mod swing;
mod transpose;

pub use arpeggiator::{ArpMode, ArpeggiatorFx};
pub use chance::ChanceFx;
pub use echo::EchoFx;
pub use harmonizer::HarmonizerFx;
pub use humanize::HumanizeFx;
pub use quantize::QuantizeFx;
pub use swing::SwingFx;
pub use transpose::TransposeFx;

use serde::{Deserialize, Serialize};

/// A MIDI event for FX processing
#[derive(Debug, Clone, Copy)]
pub struct MidiEvent {
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
    pub sample_offset: u32,
    pub is_note_on: bool,
}

/// Parameter for MIDI effects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiFxParam {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

impl MidiFxParam {
    pub fn new(name: &str, value: f32, min: f32, max: f32) -> Self {
        Self { name: name.to_string(), value, min, max }
    }
}

/// Trait for MIDI effects
pub trait MidiFx: Send {
    fn name(&self) -> &str;
    fn process(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent>;
    fn get_params(&self) -> &[MidiFxParam];
    fn set_param(&mut self, name: &str, value: f32);
    fn is_bypassed(&self) -> bool;
    fn set_bypass(&mut self, bypass: bool);
}

/// Implements common MidiFx boilerplate for structs with `params: Vec<MidiFxParam>` and `bypass: bool` fields.
/// Usage: `impl_midi_fx_boilerplate!(StructName, "Display Name");`
macro_rules! impl_midi_fx_boilerplate {
    ($ty:ty, $name:expr) => {
        impl super::MidiFx for $ty {
            fn name(&self) -> &str { $name }

            fn get_params(&self) -> &[MidiFxParam] { &self.params }

            fn set_param(&mut self, name: &str, value: f32) {
                if let Some(p) = self.params.iter_mut().find(|p| p.name == name) {
                    p.value = value.clamp(p.min, p.max);
                }
            }

            fn is_bypassed(&self) -> bool { self.bypass }
            fn set_bypass(&mut self, bypass: bool) { self.bypass = bypass; }

            fn process(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
                if self.bypass { return events; }
                self.process_impl(events, sample_rate, bpm)
            }
        }
    };
}

pub(crate) use impl_midi_fx_boilerplate;

/// Enum wrapper for all MIDI effects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MidiEffect {
    Transpose(TransposeFx),
    Quantize(QuantizeFx),
    Swing(SwingFx),
    Humanize(HumanizeFx),
    Chance(ChanceFx),
    Echo(EchoFx),
    Arpeggiator(ArpeggiatorFx),
    Harmonizer(HarmonizerFx),
}

impl MidiEffect {
    pub fn name(&self) -> &str {
        match self {
            Self::Transpose(fx) => fx.name(),
            Self::Quantize(fx) => fx.name(),
            Self::Swing(fx) => fx.name(),
            Self::Humanize(fx) => fx.name(),
            Self::Chance(fx) => fx.name(),
            Self::Echo(fx) => fx.name(),
            Self::Arpeggiator(fx) => fx.name(),
            Self::Harmonizer(fx) => fx.name(),
        }
    }

    pub fn process(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        match self {
            Self::Transpose(fx) => fx.process(events, sample_rate, bpm),
            Self::Quantize(fx) => fx.process(events, sample_rate, bpm),
            Self::Swing(fx) => fx.process(events, sample_rate, bpm),
            Self::Humanize(fx) => fx.process(events, sample_rate, bpm),
            Self::Chance(fx) => fx.process(events, sample_rate, bpm),
            Self::Echo(fx) => fx.process(events, sample_rate, bpm),
            Self::Arpeggiator(fx) => fx.process(events, sample_rate, bpm),
            Self::Harmonizer(fx) => fx.process(events, sample_rate, bpm),
        }
    }

    pub fn get_params(&self) -> &[MidiFxParam] {
        match self {
            Self::Transpose(fx) => fx.get_params(),
            Self::Quantize(fx) => fx.get_params(),
            Self::Swing(fx) => fx.get_params(),
            Self::Humanize(fx) => fx.get_params(),
            Self::Chance(fx) => fx.get_params(),
            Self::Echo(fx) => fx.get_params(),
            Self::Arpeggiator(fx) => fx.get_params(),
            Self::Harmonizer(fx) => fx.get_params(),
        }
    }

    pub fn set_param(&mut self, name: &str, value: f32) {
        match self {
            Self::Transpose(fx) => fx.set_param(name, value),
            Self::Quantize(fx) => fx.set_param(name, value),
            Self::Swing(fx) => fx.set_param(name, value),
            Self::Humanize(fx) => fx.set_param(name, value),
            Self::Chance(fx) => fx.set_param(name, value),
            Self::Echo(fx) => fx.set_param(name, value),
            Self::Arpeggiator(fx) => fx.set_param(name, value),
            Self::Harmonizer(fx) => fx.set_param(name, value),
        }
    }

    pub fn is_bypassed(&self) -> bool {
        match self {
            Self::Transpose(fx) => fx.is_bypassed(),
            Self::Quantize(fx) => fx.is_bypassed(),
            Self::Swing(fx) => fx.is_bypassed(),
            Self::Humanize(fx) => fx.is_bypassed(),
            Self::Chance(fx) => fx.is_bypassed(),
            Self::Echo(fx) => fx.is_bypassed(),
            Self::Arpeggiator(fx) => fx.is_bypassed(),
            Self::Harmonizer(fx) => fx.is_bypassed(),
        }
    }

    pub fn set_bypass(&mut self, bypass: bool) {
        match self {
            Self::Transpose(fx) => fx.set_bypass(bypass),
            Self::Quantize(fx) => fx.set_bypass(bypass),
            Self::Swing(fx) => fx.set_bypass(bypass),
            Self::Humanize(fx) => fx.set_bypass(bypass),
            Self::Chance(fx) => fx.set_bypass(bypass),
            Self::Echo(fx) => fx.set_bypass(bypass),
            Self::Arpeggiator(fx) => fx.set_bypass(bypass),
            Self::Harmonizer(fx) => fx.set_bypass(bypass),
        }
    }
}

/// MIDI FX Chain
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MidiFxChain {
    pub effects: Vec<MidiEffect>,
    pub bypass_all: bool,
}

impl MidiFxChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, effect: MidiEffect) {
        if self.effects.len() < 8 {
            self.effects.push(effect);
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<MidiEffect> {
        if index < self.effects.len() {
            return Some(self.effects.remove(index));
        }
        None
    }

    pub fn process(&mut self, mut events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        if self.bypass_all { return events; }

        for effect in &mut self.effects {
            if !effect.is_bypassed() {
                events = effect.process(events, sample_rate, bpm);
            }
        }
        events
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}
