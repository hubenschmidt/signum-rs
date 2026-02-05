//! Quantize MIDI effect

use serde::{Deserialize, Serialize};

use super::{impl_midi_fx_boilerplate, MidiEvent, MidiFxParam};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizeFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
}

impl Default for QuantizeFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("grid", 4.0, 1.0, 32.0),
                MidiFxParam::new("strength", 100.0, 0.0, 100.0),
            ],
            bypass: false,
        }
    }
}

impl QuantizeFx {
    fn process_impl(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        let grid = self.params[0].value;
        let strength = self.params[1].value / 100.0;

        let samples_per_beat = (sample_rate as f64 * 60.0 / bpm) as u32;
        let grid_samples = samples_per_beat * 4 / grid as u32;

        events.into_iter().map(|mut e| {
            let nearest_grid = ((e.sample_offset as f64 / grid_samples as f64).round() * grid_samples as f64) as u32;
            let diff = nearest_grid as i32 - e.sample_offset as i32;
            e.sample_offset = (e.sample_offset as i32 + (diff as f32 * strength) as i32).max(0) as u32;
            e
        }).collect()
    }
}

impl_midi_fx_boilerplate!(QuantizeFx, "Quantize");
