//! Swing MIDI effect

use serde::{Deserialize, Serialize};

use super::{impl_midi_fx_boilerplate, MidiEvent, MidiFxParam};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwingFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
}

impl Default for SwingFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("amount", 50.0, 0.0, 100.0),
                MidiFxParam::new("grid", 8.0, 4.0, 16.0),
            ],
            bypass: false,
        }
    }
}

impl SwingFx {
    fn process_impl(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        let amount = self.params[0].value / 100.0;
        let grid = self.params[1].value;

        let samples_per_beat = (sample_rate as f64 * 60.0 / bpm) as u32;
        let grid_samples = samples_per_beat * 4 / grid as u32;
        let swing_ratio = 0.5 + (amount - 0.5) * 0.33;

        events.into_iter().map(|mut e| {
            let grid_pos = e.sample_offset / grid_samples;
            if grid_pos % 2 == 1 {
                let swing_offset = ((swing_ratio as f64 - 0.5) * grid_samples as f64) as i32;
                e.sample_offset = (e.sample_offset as i32 + swing_offset).max(0) as u32;
            }
            e
        }).collect()
    }
}

impl_midi_fx_boilerplate!(SwingFx, "Swing");
