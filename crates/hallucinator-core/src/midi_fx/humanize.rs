//! Humanize MIDI effect

use serde::{Deserialize, Serialize};

use super::{impl_midi_fx_boilerplate, MidiEvent, MidiFxParam};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanizeFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
    rng_state: u64,
}

impl Default for HumanizeFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("timing", 10.0, 0.0, 50.0),
                MidiFxParam::new("velocity", 10.0, 0.0, 30.0),
            ],
            bypass: false,
            rng_state: 12345,
        }
    }
}

impl HumanizeFx {
    fn next_random(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((self.rng_state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn process_impl(&mut self, events: Vec<MidiEvent>, sample_rate: f32, _bpm: f64) -> Vec<MidiEvent> {
        let timing_ms = self.params[0].value;
        let vel_var = self.params[1].value;
        let timing_samples = (timing_ms / 1000.0 * sample_rate) as i32;

        events.into_iter().map(|mut e| {
            let time_offset = (self.next_random() * timing_samples as f32) as i32;
            let vel_offset = (self.next_random() * vel_var) as i8;
            e.sample_offset = (e.sample_offset as i32 + time_offset).max(0) as u32;
            e.velocity = (e.velocity as i16 + vel_offset as i16).clamp(1, 127) as u8;
            e
        }).collect()
    }
}

impl_midi_fx_boilerplate!(HumanizeFx, "Humanize");
