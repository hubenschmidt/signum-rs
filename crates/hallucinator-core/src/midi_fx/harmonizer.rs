//! Harmonizer MIDI effect

use serde::{Deserialize, Serialize};

use super::{impl_midi_fx_boilerplate, MidiEvent, MidiFxParam};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmonizerFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
}

impl Default for HarmonizerFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("interval1", 4.0, -12.0, 12.0),
                MidiFxParam::new("interval2", 7.0, -12.0, 12.0),
                MidiFxParam::new("voices", 2.0, 0.0, 2.0),
            ],
            bypass: false,
        }
    }
}

impl HarmonizerFx {
    fn process_impl(&mut self, events: Vec<MidiEvent>, _sample_rate: f32, _bpm: f64) -> Vec<MidiEvent> {
        let interval1 = self.params[0].value as i8;
        let interval2 = self.params[1].value as i8;
        let voices = self.params[2].value as u8;

        let mut result = events.clone();

        for event in &events {
            if !event.is_note_on { continue; }

            if voices >= 1 {
                let pitch1 = (event.pitch as i16 + interval1 as i16).clamp(0, 127) as u8;
                result.push(MidiEvent { pitch: pitch1, ..*event });
            }
            if voices >= 2 {
                let pitch2 = (event.pitch as i16 + interval2 as i16).clamp(0, 127) as u8;
                result.push(MidiEvent { pitch: pitch2, ..*event });
            }
        }

        result
    }
}

impl_midi_fx_boilerplate!(HarmonizerFx, "Harmonizer");
