//! Echo (MIDI delay/repeat) effect

use serde::{Deserialize, Serialize};

use super::{impl_midi_fx_boilerplate, MidiEvent, MidiFxParam};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EchoFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
}

impl Default for EchoFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("delay", 4.0, 1.0, 16.0),
                MidiFxParam::new("feedback", 3.0, 1.0, 8.0),
                MidiFxParam::new("decay", 70.0, 0.0, 100.0),
            ],
            bypass: false,
        }
    }
}

impl EchoFx {
    fn process_impl(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        let delay_div = self.params[0].value;
        let repeats = self.params[1].value as u32;
        let decay = self.params[2].value / 100.0;

        let samples_per_beat = (sample_rate as f64 * 60.0 / bpm) as u32;
        let delay_samples = samples_per_beat * 4 / delay_div as u32;

        let mut result = events.clone();

        for event in &events {
            if !event.is_note_on { continue; }

            let mut vel = event.velocity as f32;
            for i in 1..=repeats {
                vel *= decay;
                if vel < 1.0 { break; }

                result.push(MidiEvent {
                    pitch: event.pitch,
                    velocity: vel as u8,
                    channel: event.channel,
                    sample_offset: event.sample_offset + delay_samples * i,
                    is_note_on: true,
                });
            }
        }

        result.sort_by_key(|e| e.sample_offset);
        result
    }
}

impl_midi_fx_boilerplate!(EchoFx, "Echo");
