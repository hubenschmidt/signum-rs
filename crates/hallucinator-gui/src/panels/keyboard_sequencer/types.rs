//! Types, constants, and color palette for the keyboard sequencer panel.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use egui::{Color32, Key};
use hallucinator_core::ScaleMode;

// -- Key mappings --

/// Drum step keys (top row: 1-0, -, =)
pub(super) const DRUM_KEYS: [Key; 12] = [
    Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6,
    Key::Num7, Key::Num8, Key::Num9, Key::Num0, Key::Minus, Key::Plus,
];

/// Octave 3 keys (Q row)
pub(super) const OCTAVE_3_KEYS: [Key; 12] = [
    Key::Q, Key::W, Key::E, Key::R, Key::T, Key::Y,
    Key::U, Key::I, Key::O, Key::P, Key::OpenBracket, Key::CloseBracket,
];

/// Octave 4 keys (A row)
pub(super) const OCTAVE_4_KEYS: [Key; 12] = [
    Key::A, Key::S, Key::D, Key::F, Key::G, Key::H,
    Key::J, Key::K, Key::L, Key::Semicolon, Key::Quote, Key::Backslash,
];

/// Octave 5 keys (Z row)
pub(super) const OCTAVE_5_KEYS: [Key; 12] = [
    Key::Z, Key::X, Key::C, Key::V, Key::B, Key::N,
    Key::M, Key::Comma, Key::Period, Key::Slash, Key::Questionmark, Key::Enter,
];

// -- Note data --

/// Note names for display
pub(super) const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Which notes are black keys (sharps/flats)
pub(super) const IS_BLACK_KEY: [bool; 12] = [false, true, false, true, false, false, true, false, true, false, true, false];

/// Drum key labels
pub(super) const DRUM_KEY_LABELS: [&str; 12] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "="];

/// All scale modes for selector
pub(super) const ALL_SCALES: [ScaleMode; 12] = [
    ScaleMode::Chromatic, ScaleMode::Major, ScaleMode::Minor,
    ScaleMode::Dorian, ScaleMode::Phrygian, ScaleMode::Lydian,
    ScaleMode::Mixolydian, ScaleMode::Locrian, ScaleMode::HarmonicMinor,
    ScaleMode::MelodicMinor, ScaleMode::Pentatonic, ScaleMode::Blues,
];

// -- Factory Rat color palette --

pub(super) const PAD_BG: Color32 = Color32::from_rgb(38, 38, 42);
pub(super) const PAD_BORDER: Color32 = Color32::from_rgb(55, 55, 60);
pub(super) const PAD_ACTIVE: Color32 = Color32::from_rgb(220, 195, 90);
pub(super) const PAD_ACTIVE_STEP: Color32 = Color32::from_rgb(255, 235, 130);
pub(super) const PAD_CURRENT: Color32 = Color32::from_rgb(70, 68, 50);
pub(super) const PAD_PRESSED: Color32 = Color32::from_rgb(140, 200, 240);
pub(super) const PAD_BLACK: Color32 = Color32::from_rgb(28, 28, 32);
pub(super) const PANEL_BG: Color32 = Color32::from_rgb(22, 22, 26);
pub(super) const LABEL_DIM: Color32 = Color32::from_rgb(120, 120, 130);
pub(super) const LABEL_BRIGHT: Color32 = Color32::from_rgb(210, 210, 215);

// -- Layout --

/// Layout sizes for docked vs floating mode
pub(crate) struct PadLayout {
    pub(crate) size: f32,
    pub(crate) spacing: f32,
    pub(crate) radius: f32,
    pub(crate) label_w: f32,
    pub(crate) font_pad: f32,
    pub(crate) glow_inset: f32,
}

pub(crate) const DOCKED: PadLayout = PadLayout {
    size: 28.0, spacing: 2.0, radius: 4.0, label_w: 26.0,
    font_pad: 10.0, glow_inset: 3.0,
};
pub(crate) const FLOATING: PadLayout = PadLayout {
    size: 72.0, spacing: 5.0, radius: 8.0, label_w: 40.0,
    font_pad: 18.0, glow_inset: 5.0,
};

pub(super) fn truncate_label(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    s[..max].to_string()
}

// -- Data models --

/// A single layer within a drum step
#[derive(Clone)]
pub struct DrumLayer {
    pub sample_name: Option<String>,
    pub active: bool,
}

impl Default for DrumLayer {
    fn default() -> Self {
        Self { sample_name: None, active: false }
    }
}

/// A single drum step with up to 12 sample layers
#[derive(Clone)]
pub struct DrumStep {
    pub active: bool,
    pub layers: [DrumLayer; 12],
}

impl Default for DrumStep {
    fn default() -> Self {
        Self {
            active: false,
            layers: std::array::from_fn(|_| DrumLayer::default()),
        }
    }
}

impl DrumStep {
    /// Returns true if any layer has a sample loaded
    pub fn has_any_sample(&self) -> bool {
        self.layers.iter().any(|l| l.sample_name.is_some())
    }

    /// Bitmask of which layers are active (bit N = layer N)
    pub fn active_layer_mask(&self) -> u16 {
        self.layers.iter().enumerate()
            .fold(0u16, |mask, (i, l)| mask | ((l.active as u16) << i))
    }
}

/// Action returned from keyboard sequencer
#[derive(Clone)]
pub enum KeyboardSequencerAction {
    ToggleDrumStep(usize),
    PlayNote { pitch: u8, velocity: u8 },
    StopNote { pitch: u8 },
    LoadStepSample { step: usize, layer: usize, path: PathBuf },
    PlayDrumStep { step: usize, velocity: u8, active_layers: u16 },
    CopyDrumStep { step: usize, layer: usize },
    CopyStepSample { from_step: usize, from_layer: usize, to_step: usize, to_layer: usize },
    MoveStepSample { from_step: usize, from_layer: usize, to_step: usize, to_layer: usize },
    PasteStepSample { step: usize, layer: usize, name: String, data: Arc<Vec<f32>> },
    ClearStepSample { step: usize, layer: usize },
    /// Load a sample for an entire row (layer)
    LoadRowSample { row: usize, path: PathBuf },
    /// Play a row's sample (triggered by key press)
    PlayRowSample { row: usize, velocity: u8 },
    /// Copy a row's sample to clipboard
    CopyRowSample { row: usize },
    /// Paste sample from clipboard to row
    PasteRowSample { row: usize },
    /// Clear a row's sample
    ClearRowSample { row: usize },
    /// Move sample from one row to another
    MoveRowSample { from_row: usize, to_row: usize },
    /// Toggle row enabled/muted state
    ToggleRowEnabled { row: usize },
}

/// UI interaction detected during grid drawing, processed separately for SoC
#[derive(Clone, Debug)]
pub(super) enum GridInteraction {
    /// Click on a DR row step
    DrumRowClick { step: usize },
    /// Click on expanded grid sample button
    SampleButtonClick { row: usize, ctrl: bool },
    /// Click on expanded grid step cell
    GridCellClick { step: usize, row: usize, ctrl: bool },
    /// Click on a melodic row cell
    MelodicCellClick { step: usize, row: SequencerRow },
}

/// Selection state for the sequencer grid (extracted from panel struct for SoC)
#[derive(Clone, Debug)]
pub(crate) struct SelectionState {
    /// Currently selected step index
    pub selected_step: Option<usize>,
    /// Which row is active for keyboard input
    pub active_row: SequencerRow,
    /// Active drum layer (synced when navigating to drum layer rows)
    pub active_drum_layer: usize,
    /// Multi-selected rows (for batch operations)
    pub selected_rows: HashSet<usize>,
    /// Multi-selected cells (row, step) for batch operations
    pub selected_cells: HashSet<(usize, usize)>,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            selected_step: None,
            active_row: SequencerRow::default(),
            active_drum_layer: 0,
            selected_rows: HashSet::new(),
            selected_cells: HashSet::new(),
        }
    }
}

/// Payload for dragging a drum step within the sequencer (step, layer)
#[derive(Clone)]
pub(super) struct DragStep(pub usize, pub usize);

/// Payload for dragging a row sample
#[derive(Clone)]
pub(super) struct DragRowSample(pub usize);

/// Which row of the sequencer is active for keyboard input
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SequencerRow {
    /// Drum layer in expanded grid (0-11, displayed as 1-12)
    DrumLayer(usize),
    #[default]
    Drum,
    Octave3,
    Octave4,
    Octave5,
}

impl SequencerRow {
    /// Cycle to the next row (Tab). When drum_expanded, includes drum layers.
    pub fn next(self, drum_expanded: bool) -> Self {
        match self {
            Self::DrumLayer(layer) if layer > 0 => Self::DrumLayer(layer - 1),
            Self::DrumLayer(_) => Self::Drum,
            Self::Drum => Self::Octave3,
            Self::Octave3 => Self::Octave4,
            Self::Octave4 => Self::Octave5,
            Self::Octave5 if drum_expanded => Self::DrumLayer(11),
            Self::Octave5 => Self::Drum,
        }
    }

    /// Cycle to the previous row (Shift+Tab). When drum_expanded, includes drum layers.
    pub fn prev(self, drum_expanded: bool) -> Self {
        match self {
            Self::DrumLayer(layer) if layer < 11 => Self::DrumLayer(layer + 1),
            Self::DrumLayer(_) => Self::Octave5,
            Self::Drum if drum_expanded => Self::DrumLayer(0),
            Self::Drum => Self::Octave5,
            Self::Octave3 => Self::Drum,
            Self::Octave4 => Self::Octave3,
            Self::Octave5 => Self::Octave4,
        }
    }

    /// Returns the drum layer index if this is a DrumLayer row
    pub fn drum_layer(self) -> Option<usize> {
        match self {
            Self::DrumLayer(layer) => Some(layer),
            _ => None,
        }
    }
}

/// Note repeat rate for MPC/Maschine-style triggering
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatRate {
    #[default]
    Off,
    Quarter,         // 1/4 note
    Eighth,          // 1/8 note
    EighthTriplet,   // 1/8T
    Sixteenth,       // 1/16 note
    SixteenthTriplet, // 1/16T
    ThirtySecond,    // 1/32 note
}

impl RepeatRate {
    /// Returns repeat interval in beats, or None if Off
    pub fn beats(self) -> Option<f64> {
        match self {
            Self::Off => None,
            Self::Quarter => Some(1.0),
            Self::Eighth => Some(0.5),
            Self::EighthTriplet => Some(1.0 / 3.0),
            Self::Sixteenth => Some(0.25),
            Self::SixteenthTriplet => Some(1.0 / 6.0),
            Self::ThirtySecond => Some(0.125),
        }
    }

    /// Display name for UI
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Quarter => "1/4",
            Self::Eighth => "1/8",
            Self::EighthTriplet => "1/8T",
            Self::Sixteenth => "1/16",
            Self::SixteenthTriplet => "1/16T",
            Self::ThirtySecond => "1/32",
        }
    }
}

/// All repeat rates for UI selector
pub const ALL_REPEAT_RATES: [RepeatRate; 7] = [
    RepeatRate::Off,
    RepeatRate::Quarter,
    RepeatRate::Eighth,
    RepeatRate::EighthTriplet,
    RepeatRate::Sixteenth,
    RepeatRate::SixteenthTriplet,
    RepeatRate::ThirtySecond,
];
