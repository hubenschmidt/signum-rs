# Factory Rat-Style Integrated Sequencer

A pattern-based sequencer inspired by the Squarp Factory Rat hardware sequencer, featuring real-time MIDI effects, algorithmic composition tools, and song arrangement.

## Overview

The sequencer provides a non-destructive, pattern-based workflow where each MIDI track has:
- 16 pattern slots
- Up to 8 MIDI effects in a chain
- Integration with song arrangement for pattern chaining

## Core Data Structures

### Pattern System

**Location:** `crates/signum-core/src/pattern.rs`

```rust
pub struct PatternSlot {
    pub clip: Option<MidiClip>,  // MIDI data
    pub length_bars: u8,         // 1-64 bars
    pub name: String,
}

pub struct PatternBank {
    pub patterns: [PatternSlot; 16],
    pub active_pattern: usize,
    pub queued_pattern: Option<usize>,
}
```

**Features:**
- 16 patterns per track (4x4 grid in UI)
- Pattern queuing for seamless transitions at bar boundaries
- Copy/clear operations
- Variable pattern lengths (1-64 bars)

### MIDI Effects System

**Location:** `crates/signum-core/src/midi_fx.rs`

#### Available Effects

| Effect | Parameters | Description |
|--------|-----------|-------------|
| Transpose | semitones (-48 to +48) | Shift all notes up/down |
| Quantize | grid (1-32), strength (0-100%) | Snap notes to rhythmic grid |
| Swing | amount (0-100%), grid | Shift off-beats for groove |
| Humanize | timing (0-50ms), velocity (0-30) | Add random variations |
| Chance | probability (0-100%) | Probabilistic note triggering |
| Echo | delay (grid), feedback (1-8), decay | MIDI delay/repeat |
| Arpeggiator | mode, rate, octaves, gate | Arpeggiate held notes |
| Harmonizer | interval1, interval2, voices | Add parallel harmony |

#### Arpeggiator Modes
- **Up** - Ascending order
- **Down** - Descending order
- **UpDown** - Ascending then descending
- **Random** - Randomized order
- **Order** - Input order

#### Effect Chain

```rust
pub struct MidiFxChain {
    pub effects: Vec<MidiEffect>,  // Max 8
    pub bypass_all: bool,
}
```

Effects process MIDI events in real-time, non-destructively. The chain is processed in `audio_engine.rs` during event collection.

### Song Arrangement

**Location:** `crates/signum-core/src/song.rs`

```rust
pub struct SongSection {
    pub pattern_assignments: HashMap<u64, usize>,  // track_id -> pattern
    pub length_bars: u8,
    pub repeat_count: u8,
    pub name: String,
}

pub struct SongArrangement {
    pub sections: Vec<SongSection>,
    pub current_section: usize,
    pub current_repeat: u8,
    pub mode: PlaybackMode,
}

pub enum PlaybackMode {
    Pattern,  // Loop single pattern
    Song,     // Play through arrangement
}
```

### Algorithmic Tools

**Location:** `crates/signum-core/src/algorithms.rs`

#### Euclidean Rhythm Generator

```rust
pub fn euclidean_rhythm(steps: u8, hits: u8, rotation: u8) -> Vec<bool>
```

Generates evenly-distributed rhythmic patterns using Bjorklund's algorithm.

**Examples:**
- `euclidean_rhythm(8, 3, 0)` → Cuban tresillo
- `euclidean_rhythm(8, 5, 0)` → Cuban cinquillo
- `euclidean_rhythm(16, 4, 0)` → Four-on-the-floor

#### Chord Generator

```rust
pub struct ChordGenerator {
    pub root: u8,
    pub quality: ChordQuality,
    pub voicing: Voicing,
    pub octave: i8,
    pub inversion: u8,
}
```

**Chord Qualities:** Major, Minor, Dim, Aug, Maj7, Min7, Dom7, Dim7, m7b5, Sus2, Sus4, Add9

**Voicings:** Close, Open, Drop2, Drop3, RootBass

#### Scale Modes

Major, Minor, Dorian, Phrygian, Lydian, Mixolydian, Locrian, Harmonic Minor, Melodic Minor, Pentatonic, Blues, Chromatic

#### Scale Quantization

```rust
pub fn quantize_to_scale(note: u8, root: u8, mode: ScaleMode) -> u8
```

## Track Integration

**Location:** `crates/signum-core/src/track.rs`

```rust
pub struct Track {
    // ... existing fields ...
    pub pattern_bank: PatternBank,
    pub midi_fx_chain: MidiFxChain,
}
```

## Audio Engine Integration

**Location:** `crates/signum-services/src/audio_engine.rs`

MIDI events are processed through the track's MIDI FX chain before being sent to instruments:

1. Collect raw MIDI events from pattern/clips
2. Convert to `MidiEvent` structs
3. Process through `track.midi_fx_chain.process()`
4. Queue processed events to instrument

## UI Panels

### Pattern Bank Panel

**Location:** `crates/signum-gui/src/panels/pattern_bank.rs`

- 4x4 grid of pattern slots
- Click: Select pattern for editing
- Shift+Click: Queue pattern for next bar
- Double-click: Open pattern editor
- Right-click: Clear pattern
- Drag: Copy pattern to another slot
- Visual indicators: Playing (green), Queued (amber), Selected (blue border)

### MIDI FX Rack Panel

**Location:** `crates/signum-gui/src/panels/midi_fx_rack.rs`

- Vertical list of effect slots (max 8)
- ComboBox to select effect type
- Expandable parameter panels with sliders
- Bypass toggle per effect
- Remove button
- Drag to reorder effects

### Song View Panel

**Location:** `crates/signum-gui/src/panels/song_view.rs`

- Horizontal timeline of sections
- Pattern/Song mode toggle
- Section controls: length (bars), repeat count
- Add/Duplicate/Remove sections
- Drag to reorder sections
- Double-click to jump to section
- Visual bar markers within sections

## File Summary

| File | Type | Purpose |
|------|------|---------|
| `signum-core/src/pattern.rs` | New | PatternBank, PatternSlot |
| `signum-core/src/midi_fx.rs` | New | MidiFx trait, 8 effect implementations |
| `signum-core/src/song.rs` | New | SongArrangement, SongSection, PlaybackMode |
| `signum-core/src/algorithms.rs` | New | Euclidean rhythms, chord/scale tools |
| `signum-core/src/track.rs` | Modified | Added pattern_bank, midi_fx_chain |
| `signum-core/src/lib.rs` | Modified | Export new modules |
| `signum-services/src/audio_engine.rs` | Modified | MIDI FX processing integration |
| `signum-gui/src/panels/pattern_bank.rs` | New | Pattern grid UI |
| `signum-gui/src/panels/midi_fx_rack.rs` | New | MIDI FX chain UI |
| `signum-gui/src/panels/song_view.rs` | New | Song arrangement UI |
| `signum-gui/src/panels/mod.rs` | Modified | Export new panels |

## Usage Examples

### Adding MIDI Effects to a Track

```rust
use signum_core::{MidiEffect, TransposeFx, SwingFx};

// Add transpose effect
track.midi_fx_chain.add(MidiEffect::Transpose(TransposeFx::default()));

// Add swing
let mut swing = SwingFx::default();
swing.set_param("amount", 65.0);  // 65% swing
track.midi_fx_chain.add(MidiEffect::Swing(swing));
```

### Generating Euclidean Rhythm

```rust
use signum_core::euclidean_rhythm;

let pattern = euclidean_rhythm(16, 5, 0);
// Use pattern[i] to determine if step i should have a note
```

### Building Chord Progressions

```rust
use signum_core::{ChordGenerator, ChordQuality, ScaleMode};

// I-IV-V-I in C major
let chords = [
    ChordGenerator::from_scale_degree(60, ScaleMode::Major, 1, None),
    ChordGenerator::from_scale_degree(60, ScaleMode::Major, 4, None),
    ChordGenerator::from_scale_degree(60, ScaleMode::Major, 5, None),
    ChordGenerator::from_scale_degree(60, ScaleMode::Major, 1, None),
];

for chord in &chords {
    let notes = chord.generate();  // Vec<u8> of MIDI notes
}
```
