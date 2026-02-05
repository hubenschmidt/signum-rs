//! Arpeggiator MIDI effect

use serde::{Deserialize, Serialize};

use super::{impl_midi_fx_boilerplate, MidiEvent, MidiFxParam};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArpMode {
    Up,
    Down,
    UpDown,
    Random,
    Order,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArpeggiatorFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
    held_notes: Vec<u8>,
    rng_state: u64,
}

impl Default for ArpeggiatorFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("mode", 0.0, 0.0, 4.0),
                MidiFxParam::new("rate", 8.0, 1.0, 32.0),
                MidiFxParam::new("octaves", 1.0, 1.0, 4.0),
                MidiFxParam::new("gate", 80.0, 10.0, 100.0),
            ],
            bypass: false,
            held_notes: Vec::new(),
            rng_state: 99999,
        }
    }
}

impl ArpeggiatorFx {
    fn get_mode(&self) -> ArpMode {
        match self.params[0].value as u8 {
            0 => ArpMode::Up,
            1 => ArpMode::Down,
            2 => ArpMode::UpDown,
            3 => ArpMode::Random,
            _ => ArpMode::Order,
        }
    }

    fn next_random(&mut self) -> usize {
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.rng_state >> 33) as usize
    }

    fn process_impl(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        for event in &events {
            if event.is_note_on {
                if !self.held_notes.contains(&event.pitch) {
                    self.held_notes.push(event.pitch);
                }
            } else {
                self.held_notes.retain(|&p| p != event.pitch);
            }
        }

        if self.held_notes.is_empty() { return vec![]; }

        let rate = self.params[1].value;
        let octaves = self.params[2].value as u8;
        let gate = self.params[3].value / 100.0;

        let samples_per_beat = (sample_rate as f64 * 60.0 / bpm) as u32;
        let note_samples = samples_per_beat * 4 / rate as u32;
        let gate_samples = (note_samples as f32 * gate) as u32;

        let mut sequence: Vec<u8> = Vec::new();
        let mode = self.get_mode();

        let mut sorted_notes = self.held_notes.clone();
        sorted_notes.sort();

        for oct in 0..octaves {
            let oct_offset = oct * 12;
            match mode {
                ArpMode::Up | ArpMode::UpDown | ArpMode::Order => {
                    for &note in &sorted_notes {
                        sequence.push((note as u16 + oct_offset as u16).min(127) as u8);
                    }
                }
                ArpMode::Down => {
                    for &note in sorted_notes.iter().rev() {
                        sequence.push((note as u16 + oct_offset as u16).min(127) as u8);
                    }
                }
                ArpMode::Random => {
                    for &note in &sorted_notes {
                        sequence.push((note as u16 + oct_offset as u16).min(127) as u8);
                    }
                }
            }
        }

        if mode == ArpMode::UpDown && sequence.len() > 2 {
            let down: Vec<u8> = sequence[1..sequence.len()-1].iter().rev().copied().collect();
            sequence.extend(down);
        }

        if mode == ArpMode::Random && !sequence.is_empty() {
            for i in (1..sequence.len()).rev() {
                let j = self.next_random() % (i + 1);
                sequence.swap(i, j);
            }
        }

        let mut result = Vec::new();
        let channel = events.first().map(|e| e.channel).unwrap_or(0);

        for (i, &pitch) in sequence.iter().enumerate() {
            let offset = note_samples * i as u32;
            result.push(MidiEvent {
                pitch,
                velocity: 100,
                channel,
                sample_offset: offset,
                is_note_on: true,
            });
            result.push(MidiEvent {
                pitch,
                velocity: 0,
                channel,
                sample_offset: offset + gate_samples,
                is_note_on: false,
            });
        }

        result
    }
}

impl_midi_fx_boilerplate!(ArpeggiatorFx, "Arpeggiator");
