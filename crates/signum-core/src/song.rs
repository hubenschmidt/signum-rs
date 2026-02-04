//! Song arrangement for Hapax-style pattern chaining

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A section in the song arrangement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongSection {
    /// Pattern index assignments per track (track_id -> pattern_index)
    pub pattern_assignments: HashMap<u64, usize>,
    /// Section length in bars
    pub length_bars: u8,
    /// Number of times to repeat this section
    pub repeat_count: u8,
    /// Section name (optional)
    pub name: String,
}

impl Default for SongSection {
    fn default() -> Self {
        Self {
            pattern_assignments: HashMap::new(),
            length_bars: 4,
            repeat_count: 1,
            name: String::new(),
        }
    }
}

impl SongSection {
    pub fn new(length_bars: u8) -> Self {
        Self {
            length_bars,
            ..Default::default()
        }
    }

    /// Set pattern for a track in this section
    pub fn set_pattern(&mut self, track_id: u64, pattern_idx: usize) {
        self.pattern_assignments.insert(track_id, pattern_idx);
    }

    /// Get pattern index for a track (defaults to 0)
    pub fn get_pattern(&self, track_id: u64) -> usize {
        self.pattern_assignments.get(&track_id).copied().unwrap_or(0)
    }

    /// Total bars including repeats
    pub fn total_bars(&self) -> u32 {
        self.length_bars as u32 * self.repeat_count.max(1) as u32
    }
}

/// Playback mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackMode {
    /// Play single pattern in loop
    Pattern,
    /// Play through song arrangement
    Song,
}

impl Default for PlaybackMode {
    fn default() -> Self {
        Self::Pattern
    }
}

/// Song arrangement containing sections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongArrangement {
    /// Sections in playback order
    pub sections: Vec<SongSection>,
    /// Current section index during playback
    pub current_section: usize,
    /// Current repeat index within section
    pub current_repeat: u8,
    /// Playback mode
    pub mode: PlaybackMode,
}

impl Default for SongArrangement {
    fn default() -> Self {
        Self {
            sections: vec![SongSection::default()],
            current_section: 0,
            current_repeat: 0,
            mode: PlaybackMode::Pattern,
        }
    }
}

impl SongArrangement {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new section
    pub fn add_section(&mut self, section: SongSection) {
        self.sections.push(section);
    }

    /// Insert section at index
    pub fn insert_section(&mut self, index: usize, section: SongSection) {
        let idx = index.min(self.sections.len());
        self.sections.insert(idx, section);
    }

    /// Remove section at index
    pub fn remove_section(&mut self, index: usize) -> Option<SongSection> {
        if index >= self.sections.len() || self.sections.len() <= 1 {
            return None;
        }
        Some(self.sections.remove(index))
    }

    /// Get current section
    pub fn current(&self) -> Option<&SongSection> {
        self.sections.get(self.current_section)
    }

    /// Get mutable current section
    pub fn current_mut(&mut self) -> Option<&mut SongSection> {
        self.sections.get_mut(self.current_section)
    }

    /// Advance to next section/repeat, returns true if song continues
    pub fn advance(&mut self) -> bool {
        let Some(section) = self.sections.get(self.current_section) else {
            return false;
        };

        // Check if we have more repeats
        if self.current_repeat + 1 < section.repeat_count {
            self.current_repeat += 1;
            return true;
        }

        // Move to next section
        self.current_repeat = 0;
        self.current_section += 1;

        self.current_section < self.sections.len()
    }

    /// Reset to beginning
    pub fn reset(&mut self) {
        self.current_section = 0;
        self.current_repeat = 0;
    }

    /// Total length in bars
    pub fn total_bars(&self) -> u32 {
        self.sections.iter().map(|s| s.total_bars()).sum()
    }

    /// Get section at bar position
    pub fn section_at_bar(&self, bar: u32) -> Option<(usize, &SongSection)> {
        let mut accumulated = 0u32;
        for (idx, section) in self.sections.iter().enumerate() {
            let section_bars = section.total_bars();
            if bar < accumulated + section_bars {
                return Some((idx, section));
            }
            accumulated += section_bars;
        }
        None
    }

    /// Copy section
    pub fn copy_section(&mut self, from: usize, to: usize) {
        if from >= self.sections.len() || to >= self.sections.len() {
            return;
        }
        self.sections[to] = self.sections[from].clone();
    }

    /// Duplicate section (insert copy after original)
    pub fn duplicate_section(&mut self, index: usize) {
        if index >= self.sections.len() {
            return;
        }
        let copy = self.sections[index].clone();
        self.sections.insert(index + 1, copy);
    }
}
