//! Chance MIDI effect

use serde::{Deserialize, Serialize};

use super::{impl_midi_fx_boilerplate, MidiEvent, MidiFxParam};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChanceFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
    rng_state: u64,
}

impl Default for ChanceFx {
    fn default() -> Self {
        Self {
            params: vec![MidiFxParam::new("probability", 100.0, 0.0, 100.0)],
            bypass: false,
            rng_state: 54321,
        }
    }
}

impl ChanceFx {
    fn next_random(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.rng_state >> 33) as f32 / u32::MAX as f32
    }

    fn process_impl(&mut self, events: Vec<MidiEvent>, _sample_rate: f32, _bpm: f64) -> Vec<MidiEvent> {
        let prob = self.params[0].value / 100.0;
        events.into_iter().filter(|_| self.next_random() < prob).collect()
    }
}

impl_midi_fx_boilerplate!(ChanceFx, "Chance");
