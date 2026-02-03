//! Audio and MIDI clip representations

use serde::{Deserialize, Serialize};

/// Unique identifier for clips
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClipId(pub u64);

/// A single MIDI note event
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MidiNote {
    /// MIDI note number (0-127, 60 = middle C)
    pub pitch: u8,
    /// Velocity (0-127)
    pub velocity: u8,
    /// Start position in ticks (PPQ-based)
    pub start_tick: u64,
    /// Duration in ticks
    pub duration_ticks: u64,
}

impl MidiNote {
    pub fn new(pitch: u8, velocity: u8, start_tick: u64, duration_ticks: u64) -> Self {
        Self {
            pitch,
            velocity,
            start_tick,
            duration_ticks,
        }
    }

    /// End tick (start + duration)
    pub fn end_tick(&self) -> u64 {
        self.start_tick + self.duration_ticks
    }
}

/// MIDI clip containing note events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiClip {
    pub id: ClipId,
    /// Start position in samples (timeline position)
    pub start_sample: u64,
    /// Length in samples
    pub length_samples: u64,
    /// Clip name/label
    pub name: String,
    /// Notes sorted by start_tick
    pub notes: Vec<MidiNote>,
    /// Pulses per quarter note (default 480)
    pub ppq: u16,
}

impl MidiClip {
    pub fn new(id: ClipId, length_samples: u64) -> Self {
        Self {
            id,
            start_sample: 0,
            length_samples,
            name: String::new(),
            notes: Vec::new(),
            ppq: 480,
        }
    }

    /// End position in samples
    pub fn end_sample(&self) -> u64 {
        self.start_sample + self.length_samples
    }

    /// Add a note, keeping notes sorted by start_tick
    pub fn add_note(&mut self, note: MidiNote) {
        let idx = self.notes
            .iter()
            .position(|n| n.start_tick > note.start_tick)
            .unwrap_or(self.notes.len());
        self.notes.insert(idx, note);
    }

    /// Remove note at index
    pub fn remove_note(&mut self, index: usize) -> Option<MidiNote> {
        if index < self.notes.len() {
            return Some(self.notes.remove(index));
        }
        None
    }
}

/// Audio clip on a track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioClip {
    pub id: ClipId,
    /// Start position in samples (timeline position)
    pub start_sample: u64,
    /// Length in samples
    pub length_samples: u64,
    /// Source audio data (interleaved f32 samples)
    #[serde(skip)]
    pub samples: Vec<f32>,
    /// Sample rate of the audio data
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
    /// Clip name/label
    pub name: String,
    /// Gain multiplier (1.0 = unity)
    pub gain: f32,
}

impl AudioClip {
    pub fn new(id: ClipId, samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        let length_samples = samples.len() as u64 / channels as u64;
        Self {
            id,
            start_sample: 0,
            length_samples,
            samples,
            sample_rate,
            channels,
            name: String::new(),
            gain: 1.0,
        }
    }

    /// End position in samples
    pub fn end_sample(&self) -> u64 {
        self.start_sample + self.length_samples
    }

    /// Duration in seconds
    pub fn duration_secs(&self) -> f64 {
        self.length_samples as f64 / self.sample_rate as f64
    }

    /// Get sample at timeline position (returns mono sum if stereo)
    pub fn sample_at(&self, timeline_sample: u64) -> Option<f32> {
        let clip_offset = timeline_sample.checked_sub(self.start_sample)?;

        if clip_offset >= self.length_samples {
            return None;
        }

        let frame_idx = clip_offset as usize * self.channels as usize;

        if frame_idx >= self.samples.len() {
            return None;
        }

        // Sum channels to mono
        let sum: f32 = self.samples[frame_idx..]
            .iter()
            .take(self.channels as usize)
            .sum();
        Some(sum / self.channels as f32 * self.gain)
    }
}
