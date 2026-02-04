# QWERTY Keyboard Sequencer

A keyboard-based sequencer with drum step programming and a 3-octave chromatic keyboard for polyphonic playing.

## Overview

The sequencer maps your QWERTY keyboard to musical functions:
- **Top row (1-=)**: 12-step drum sequencer
- **Bottom 3 rows**: 3-octave chromatic keyboard (C3-B5)

## Keyboard Layout

```
1 2 3 4 5 6 7 8 9 0 - =    → 12 drum steps (toggle on/off)
Q W E R T Y U I O P [ ]    → C3 C# D D# E F F# G G# A A# B (octave 3)
A S D F G H J K L ; ' \    → C4 C# D D# E F F# G G# A A# B (octave 4)
Z X C V B N M , . / ? +    → C5 C# D D# E F F# G G# A A# B (octave 5)
```

## Drum Row (Top)

Press number keys 1-0, -, = to toggle drum steps on/off.

| Key | Step |
|-----|------|
| 1-9 | Steps 1-9 |
| 0 | Step 10 |
| - | Step 11 |
| = | Step 12 |

### Visual Indicators
- **Filled** - Step is active (will play)
- **Outlined** - Step is inactive
- **Highlighted** - Current playhead position

## Melodic Rows (Bottom 3)

Play notes by pressing keys. Hold multiple keys for chords.

### Octave Layout
| Row | Keys | Notes | MIDI Range |
|-----|------|-------|------------|
| Q row | Q W E R T Y U I O P [ ] | C3-B3 | 48-59 |
| A row | A S D F G H J K L ; ' \ | C4-B4 | 60-71 |
| Z row | Z X C V B N M , . / | C5-B5 | 72-83 |

### Note Names
```
C  C# D  D# E  F  F# G  G# A  A# B
Q  W  E  R  T  Y  U  I  O  P  [  ]
```

### Polyphonic Playing
- Hold multiple keys simultaneously to play chords
- Example: Q + T + I = C major chord (C + E + G)
- Release keys to stop notes

## MIDI FX Rack

Real-time, non-destructive MIDI processing applied to notes you play.

### Available Effects

| Effect | Parameters | Description |
|--------|-----------|-------------|
| **Transpose** | semitones (-48 to +48) | Shift all notes up/down |
| **Quantize** | grid, strength (0-100%) | Snap notes to rhythmic grid |
| **Swing** | amount (0-100%), grid | Shift off-beats for groove |
| **Humanize** | timing (0-50ms), velocity (0-30) | Add random variations |
| **Chance** | probability (0-100%) | Probabilistic note triggering |
| **Echo** | delay (grid), feedback (1-8), decay | MIDI delay/repeat |
| **Arpeggiator** | mode, rate, octaves, gate | Arpeggiate held notes |
| **Harmonizer** | interval1, interval2, voices | Add parallel harmony |

### Controls
- **+** button - Add selected effect type
- Click effect row - Expand/collapse parameters
- **B** button - Bypass effect
- **x** button - Remove effect
- Drag effects - Reorder chain

## Song View

Arrange patterns into sections for full song playback.

### Playback Modes
- **Pattern** - Loop the current pattern
- **Song** - Play through arrangement sections

### Section Controls
- **+ Add Section** - Create new section
- Click section - Select for editing
- **Length** slider - Set section length (1-64 bars)
- **Repeat** slider - Set repeat count (1-16x)

## Tips

- Use the **Arpeggiator** effect to turn held chords into melodic patterns
- Stack **Transpose** effects for octave layering
- The **Chance** effect at 50% creates variation in drum patterns
- Hold Q + T + I for a C major chord, Q + R + U for C minor
