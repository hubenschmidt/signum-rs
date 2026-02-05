//! Multi-sample drum kit — each slot holds a different sample, triggered by MIDI note

use std::sync::Arc;

use crate::audio_effects::{AudioInstrument, EffectParam};

const MAX_SLOTS: usize = 144; // 12 steps × 12 layers
const MAX_VOICES: usize = 32;
const MAX_BLOCK_SIZE: usize = 4096;
/// Base MIDI note for slot 0 (C1, same as 808 kick)
const BASE_NOTE: u8 = 36;

/// A loaded sample assigned to a kit slot.
pub struct SampleSlot {
    pub name: String,
    pub data: Arc<Vec<f32>>,
}

/// A single playback voice.
struct KitVoice {
    active: bool,
    slot: usize,
    position: f64,
    velocity: f32,
    age: usize,
}

impl KitVoice {
    fn new() -> Self {
        Self { active: false, slot: 0, position: 0.0, velocity: 0.0, age: 0 }
    }

    fn trigger(&mut self, slot: usize, velocity: u8) {
        self.active = true;
        self.slot = slot;
        self.position = 0.0;
        self.velocity = velocity as f32 / 127.0;
        self.age = 0;
    }

    fn tick(&mut self, data: &[f32]) -> f32 {
        if !self.active {
            return 0.0;
        }

        self.age += 1;

        let idx = self.position as usize;
        if idx >= data.len().saturating_sub(1) {
            self.active = false;
            return 0.0;
        }

        let frac = (self.position - idx as f64) as f32;
        let s0 = data[idx];
        let s1 = data[idx + 1];
        let sample = s0 + frac * (s1 - s0);

        self.position += 1.0; // 1:1 playback, no pitch shift

        sample * self.velocity
    }
}

/// Multi-sample drum kit instrument.
pub struct SampleKit {
    slots: Vec<Option<SampleSlot>>,
    voices: Vec<KitVoice>,
    pending_events: Vec<(u8, u8, u32)>,
    output_left: Vec<f32>,
    output_right: Vec<f32>,
    sample_rate: f32,
    master: f32,
    param_cache: Vec<EffectParam>,
}

impl std::fmt::Debug for SampleKit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let loaded = self.slots.iter().filter(|s| s.is_some()).count();
        f.debug_struct("SampleKit")
            .field("loaded_slots", &loaded)
            .field("active_voices", &self.voices.iter().filter(|v| v.active).count())
            .finish()
    }
}

impl SampleKit {
    pub fn new(sample_rate: f32) -> Self {
        let mut slots = Vec::with_capacity(MAX_SLOTS);
        slots.resize_with(MAX_SLOTS, || None);

        Self {
            slots,
            voices: (0..MAX_VOICES).map(|_| KitVoice::new()).collect(),
            pending_events: Vec::new(),
            output_left: vec![0.0; MAX_BLOCK_SIZE],
            output_right: vec![0.0; MAX_BLOCK_SIZE],
            sample_rate,
            master: 0.8,
            param_cache: vec![EffectParam::new("master", 0.8, 0.0, 1.0, "")],
        }
    }

    /// Read-only access to all slots.
    pub fn slots(&self) -> &[Option<SampleSlot>] {
        &self.slots
    }

    /// Assign a sample to a slot (0-based index).
    pub fn set_slot(&mut self, index: usize, name: String, data: Arc<Vec<f32>>) {
        if index >= MAX_SLOTS {
            return;
        }
        self.slots[index] = Some(SampleSlot { name, data });
    }

    /// Remove a sample from a slot.
    pub fn clear_slot(&mut self, index: usize) {
        if index < MAX_SLOTS {
            self.slots[index] = None;
        }
    }

    /// Trigger active layers for a given step. `active_layers` is a bitmask (bit N = layer N).
    pub fn trigger_step(&mut self, step: usize, velocity: u8, active_layers: u16) {
        let base = step * 12;
        for layer in 0..12u16 {
            if active_layers & (1 << layer) == 0 { continue; }
            let slot = base + layer as usize;
            if slot < self.slots.len() && self.slots[slot].is_some() {
                self.trigger_slot(slot, velocity);
            }
        }
    }

    fn trigger_slot(&mut self, slot: usize, velocity: u8) {
        if slot >= self.slots.len() || self.slots[slot].is_none() {
            return;
        }

        // Voice stealing: same slot first, then inactive, then oldest
        let vi = self.voices.iter().position(|v| v.active && v.slot == slot)
            .or_else(|| self.voices.iter().position(|v| !v.active))
            .unwrap_or_else(|| {
                self.voices.iter()
                    .enumerate()
                    .max_by_key(|(_, v)| v.age)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            });

        self.voices[vi].trigger(slot, velocity);
    }

    fn trigger_from_midi(&mut self, pitch: u8, velocity: u8) {
        let slot = ((pitch.wrapping_sub(BASE_NOTE)) as usize) % MAX_SLOTS;
        self.trigger_slot(slot, velocity);
    }
}

impl AudioInstrument for SampleKit {
    fn name(&self) -> &str {
        "Sample Kit"
    }

    fn queue_note_on(&mut self, pitch: u8, velocity: u8, _channel: u8, sample_offset: u32) {
        self.pending_events.push((pitch, velocity, sample_offset));
    }

    fn queue_note_off(&mut self, _pitch: u8, _velocity: u8, _channel: u8, _sample_offset: u32) {
        // Kit samples are one-shot, no note-off needed
    }

    fn all_notes_off(&mut self) {
        for voice in &mut self.voices {
            voice.active = false;
        }
    }

    fn process(&mut self, num_frames: usize) -> (&[f32], &[f32]) {
        let frames = num_frames.min(MAX_BLOCK_SIZE);

        self.output_left[..frames].fill(0.0);
        self.output_right[..frames].fill(0.0);

        self.pending_events.sort_by_key(|e| e.2);

        for frame_idx in 0..frames {
            while let Some(&(pitch, velocity, offset)) = self.pending_events.first() {
                if offset as usize > frame_idx {
                    break;
                }
                self.pending_events.remove(0);
                self.trigger_from_midi(pitch, velocity);
            }

            let mut mix = 0.0_f32;
            for voice in &mut self.voices {
                if !voice.active {
                    continue;
                }
                let slot_data = match &self.slots[voice.slot] {
                    Some(slot) => &slot.data,
                    None => {
                        voice.active = false;
                        continue;
                    }
                };
                mix += voice.tick(slot_data);
            }

            let out = (mix * self.master).clamp(-1.0, 1.0);
            self.output_left[frame_idx] = out;
            self.output_right[frame_idx] = out;
        }

        self.pending_events.retain(|e| e.2 as usize >= frames);
        for event in &mut self.pending_events {
            event.2 -= frames as u32;
        }

        (&self.output_left[..frames], &self.output_right[..frames])
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    fn get_params(&self) -> &[EffectParam] {
        &self.param_cache
    }

    fn set_param(&mut self, name: &str, value: f32) {
        if name == "master" {
            self.master = value;
            self.param_cache = vec![EffectParam::new("master", value, 0.0, 1.0, "")];
        }
    }

    fn set_param_by_index(&mut self, index: usize, value: f64) {
        if index == 0 {
            self.master = value as f32;
            self.param_cache = vec![EffectParam::new("master", value as f32, 0.0, 1.0, "")];
        }
    }

    fn is_drum(&self) -> bool {
        true
    }
}
