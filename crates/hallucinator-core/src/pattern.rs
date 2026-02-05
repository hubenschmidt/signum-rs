//! Pattern bank for Factory Rat-style sequencing

use serde::{Deserialize, Serialize};
use crate::clip::MidiClip;

/// A single pattern slot in the bank
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatternSlot {
    /// Optional MIDI clip data for this pattern
    #[serde(skip)]
    pub clip: Option<MidiClip>,
    /// Length in bars (1-64)
    pub length_bars: u8,
    /// Pattern name (e.g., "Intro", "Verse")
    pub name: String,
}

impl PatternSlot {
    pub fn new(length_bars: u8) -> Self {
        Self {
            clip: None,
            length_bars,
            name: String::new(),
        }
    }

    pub fn with_clip(clip: MidiClip, length_bars: u8) -> Self {
        Self {
            clip: Some(clip),
            length_bars,
            name: String::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.clip.as_ref().map_or(true, |c| c.notes.is_empty())
    }
}

/// Pattern bank holding 16 patterns per track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternBank {
    /// 16 pattern slots
    pub patterns: [PatternSlot; 16],
    /// Currently active pattern index for editing
    pub active_pattern: usize,
    /// Pattern queued for next bar (None = no change)
    pub queued_pattern: Option<usize>,
}

impl Default for PatternBank {
    fn default() -> Self {
        Self {
            patterns: std::array::from_fn(|_| PatternSlot::new(4)),
            active_pattern: 0,
            queued_pattern: None,
        }
    }
}

impl PatternBank {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently active pattern slot
    pub fn active(&self) -> &PatternSlot {
        &self.patterns[self.active_pattern]
    }

    /// Get mutable reference to active pattern
    pub fn active_mut(&mut self) -> &mut PatternSlot {
        &mut self.patterns[self.active_pattern]
    }

    /// Set active pattern index (clamped to 0-15)
    pub fn set_active(&mut self, index: usize) {
        self.active_pattern = index.min(15);
    }

    /// Queue pattern change for next bar boundary
    pub fn queue_pattern(&mut self, index: usize) {
        self.queued_pattern = Some(index.min(15));
    }

    /// Process queued pattern change (call at bar boundary)
    pub fn process_queue(&mut self) -> bool {
        let Some(queued) = self.queued_pattern.take() else {
            return false;
        };
        self.active_pattern = queued;
        true
    }

    /// Copy pattern from one slot to another
    pub fn copy_pattern(&mut self, from: usize, to: usize) {
        if from >= 16 || to >= 16 || from == to {
            return;
        }
        self.patterns[to] = self.patterns[from].clone();
    }

    /// Clear a pattern slot
    pub fn clear_pattern(&mut self, index: usize) {
        if index >= 16 {
            return;
        }
        self.patterns[index] = PatternSlot::new(4);
    }
}
