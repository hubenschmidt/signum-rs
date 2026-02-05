//! Transpose MIDI effect

use serde::{Deserialize, Serialize};

use super::{impl_midi_fx_boilerplate, MidiEvent, MidiFxParam};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransposeFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
}

impl Default for TransposeFx {
    fn default() -> Self {
        Self {
            params: vec![MidiFxParam::new("semitones", 0.0, -48.0, 48.0)],
            bypass: false,
        }
    }
}

impl TransposeFx {
    fn process_impl(&mut self, events: Vec<MidiEvent>, _sample_rate: f32, _bpm: f64) -> Vec<MidiEvent> {
        let semitones = self.params[0].value as i32;
        events.into_iter().map(|mut e| {
            e.pitch = (e.pitch as i32 + semitones).clamp(0, 127) as u8;
            e
        }).collect()
    }
}

impl_midi_fx_boilerplate!(TransposeFx, "Transpose");
