//! Track representation

use serde::{Deserialize, Serialize};
use crate::clip::{AudioClip, ClipId, MidiClip};
use crate::midi_fx::MidiFxChain;
use crate::pattern::PatternBank;

/// Unique identifier for tracks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackId(pub u64);

/// Track type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    Audio,
    Midi,
    Master,
}

/// A track in the timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub kind: TrackKind,
    pub name: String,
    /// Volume (0.0 to 1.0+)
    pub volume: f32,
    /// Pan (-1.0 left, 0.0 center, 1.0 right)
    pub pan: f32,
    /// Muted state
    pub mute: bool,
    /// Solo state
    pub solo: bool,
    /// Armed for recording
    pub armed: bool,
    /// Audio clips on this track
    #[serde(skip)]
    pub clips: Vec<AudioClip>,
    /// MIDI clips on this track
    #[serde(skip)]
    pub midi_clips: Vec<MidiClip>,
    /// Assigned VST instrument ID (for MIDI tracks)
    pub instrument_id: Option<u64>,
    /// Assigned effect chain ID (for per-track effects)
    pub effect_chain_id: Option<u64>,
    /// Pattern bank (16 patterns per track, Factory Rat-style)
    #[serde(default)]
    pub pattern_bank: PatternBank,
    /// MIDI FX chain (up to 8 effects)
    #[serde(default)]
    pub midi_fx_chain: MidiFxChain,
}

impl Track {
    pub fn new(id: TrackId, kind: TrackKind, name: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            name: name.into(),
            volume: 1.0,
            pan: 0.0,
            mute: false,
            solo: false,
            armed: false,
            clips: Vec::new(),
            midi_clips: Vec::new(),
            instrument_id: None,
            effect_chain_id: None,
            pattern_bank: PatternBank::default(),
            midi_fx_chain: MidiFxChain::default(),
        }
    }

    pub fn add_clip(&mut self, clip: AudioClip) {
        self.clips.push(clip);
    }

    pub fn remove_clip(&mut self, clip_id: ClipId) -> Option<AudioClip> {
        let pos = self.clips.iter().position(|c| c.id == clip_id)?;
        Some(self.clips.remove(pos))
    }

    pub fn get_clip(&self, clip_id: ClipId) -> Option<&AudioClip> {
        self.clips.iter().find(|c| c.id == clip_id)
    }

    pub fn get_clip_mut(&mut self, clip_id: ClipId) -> Option<&mut AudioClip> {
        self.clips.iter_mut().find(|c| c.id == clip_id)
    }

    /// Get audio sample at timeline position (summed from all clips)
    pub fn sample_at(&self, timeline_sample: u64) -> f32 {
        if self.mute {
            return 0.0;
        }

        let raw: f32 = self.clips
            .iter()
            .filter_map(|clip| clip.sample_at(timeline_sample))
            .sum();

        raw * self.volume
    }

    pub fn add_midi_clip(&mut self, clip: MidiClip) {
        self.midi_clips.push(clip);
    }

    pub fn remove_midi_clip(&mut self, clip_id: ClipId) -> Option<MidiClip> {
        let pos = self.midi_clips.iter().position(|c| c.id == clip_id)?;
        Some(self.midi_clips.remove(pos))
    }

    pub fn get_midi_clip(&self, clip_id: ClipId) -> Option<&MidiClip> {
        self.midi_clips.iter().find(|c| c.id == clip_id)
    }

    pub fn get_midi_clip_mut(&mut self, clip_id: ClipId) -> Option<&mut MidiClip> {
        self.midi_clips.iter_mut().find(|c| c.id == clip_id)
    }
}
