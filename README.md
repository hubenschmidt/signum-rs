# Hallucinator

A Rust-based DAW (Digital Audio Workstation) built with `egui` for the GUI and `cpal` for real-time audio I/O.

## Features

- **Multi-track timeline** with MIDI and audio clip arrangement
- **VST3 plugin hosting** for external instruments and effects
- **Native instruments** — TR-808 drum synth, polyphonic sampler, sample kits
- **Native effects** — gain, high-pass, low-pass, compressor, delay, reverb
- **MIDI FX rack** — transpose, quantize, swing, humanize, chance, echo, arpeggiator, harmonizer
- **Piano roll & drum roll** editors
- **Section-based song view** for high-level arrangement

## Factory Rat (Keyboard Sequencer)

A QWERTY keyboard-driven pad sequencer with scale-aware melodic input.

### Layout

| Row         | Keys         | Function                              |
|-------------|--------------|---------------------------------------|
| Top         | `1 2 3 … 0 - =` | Drum pads (4/6/8/12 configurable steps) |
| QWERTY      | `Q W E R …`     | Melodic octave 3 (C5)                |
| ASDF        | `A S D F …`     | Melodic octave 2 (C4)                |
| ZXCV        | `Z X C V …`     | Melodic octave 1 (C3)                |

### Capabilities

- **12 scale modes** — chromatic, major, minor, dorian, phrygian, lydian, mixolydian, locrian, harmonic minor, melodic minor, pentatonic, blues
- **Selectable root note** for non-chromatic scales
- **12 sample layers per drum step** — each step can trigger multiple samples in parallel
- **MPC-style note repeat** — 1/4, 1/8, 1/8T, 1/16, 1/16T, 1/32
- **Row muting** and per-row enable/disable toggles
- **Loop snapping** to the arrange view loop region
- **Tab navigation** across drum layers, drum row, and melodic octave rows
- **Floating or docked** layout with size scaling

## Tech Stack

- **Audio**: `cpal`, `fundsp`, `rubato`
- **GUI**: `egui` / `eframe`
- **VST3**: `rack` crate
- **Platform**: Linux (X11)
