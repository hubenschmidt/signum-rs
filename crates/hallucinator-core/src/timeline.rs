//! Timeline containing tracks

use serde::{Deserialize, Serialize};
use crate::track::{Track, TrackId, TrackKind};
use crate::transport::Transport;

/// The main timeline containing all tracks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Timeline {
    pub tracks: Vec<Track>,
    pub transport: Transport,
    next_track_id: u64,
}

impl Timeline {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            tracks: Vec::new(),
            transport: Transport::new(sample_rate),
            next_track_id: 1,
        }
    }

    pub fn add_track(&mut self, kind: TrackKind, name: impl Into<String>) -> TrackId {
        let id = TrackId(self.next_track_id);
        self.next_track_id += 1;
        let track = Track::new(id, kind, name);
        self.tracks.push(track);
        id
    }

    pub fn remove_track(&mut self, id: TrackId) -> Option<Track> {
        let pos = self.tracks.iter().position(|t| t.id == id)?;
        Some(self.tracks.remove(pos))
    }

    pub fn get_track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn get_track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// Check if any track is soloed
    pub fn has_solo(&self) -> bool {
        self.tracks.iter().any(|t| t.solo)
    }

    /// Get mixed audio sample at timeline position
    pub fn sample_at(&self, timeline_sample: u64) -> f32 {
        let has_solo = self.has_solo();

        self.tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio)
            .filter(|t| !has_solo || t.solo)
            .map(|t| t.sample_at(timeline_sample))
            .sum()
    }

    /// Total duration in samples (end of last clip)
    pub fn duration_samples(&self) -> u64 {
        self.tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .map(|c| c.end_sample())
            .max()
            .unwrap_or(0)
    }

    /// Total duration in seconds
    pub fn duration_secs(&self) -> f64 {
        self.duration_samples() as f64 / self.transport.sample_rate as f64
    }
}
