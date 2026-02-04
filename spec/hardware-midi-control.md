# External Hardware MIDI Control

Route MIDI from signum tracks to external hardware synths (e.g., Dave Smith Prophet Rev2) via USB-MIDI or MIDI interfaces.

## Current State

- MIDI is internal only: clips → MIDI FX chain → VST3/native instrument → audio
- No `midir` or MIDI I/O library in dependencies
- No MIDI port enumeration or selection

## Architecture

```
Track MIDI Events
  ↓
MIDI FX Chain (Transpose, Swing, etc.)
  ↓
┌─────────────────────────────┐
│ Instrument Router           │
│  ├─ VST3 → internal audio   │
│  ├─ Drum808 → internal audio │
│  └─ ExternalMidi → MIDI port │
└─────────────────────────────┘
  ↓ (ExternalMidi path)
midir → USB-MIDI → Prophet Rev2
```

## Implementation

### 1. Add `midir` dependency

**File:** `crates/signum-services/Cargo.toml`
```toml
midir = "0.10"
```

### 2. MIDI output service

**File:** `crates/signum-services/src/midi_output.rs` (NEW)

```rust
pub struct MidiOutputService {
    connections: HashMap<u64, Arc<Mutex<MidiConnection>>>,  // track_id → connection
}

pub struct MidiPortInfo {
    pub index: usize,
    pub name: String,
}

pub struct MidiConnection {
    conn: MidiOutputConnection,
    channel: u8,  // 0-15
}
```

**Methods:**
- `list_ports()` → enumerate available MIDI output ports
- `connect(track_id, port_index, channel)` → open connection
- `disconnect(track_id)` → close connection, send all-notes-off
- `get_connection(track_id)` → get handle for sending
- `panic()` → all-notes-off on all connections

### 3. ExternalMidi instrument type

**File:** `crates/signum-services/src/audio_effects/mod.rs`

Add variant to Instrument enum:
```rust
pub enum Instrument {
    Vst3(Vst3Instrument),
    Drum808(Drum808),
    ExternalMidi(ExternalMidiInstrument),  // NEW
}
```

`ExternalMidiInstrument` wraps an `Arc<Mutex<MidiConnection>>` and implements `queue_note_on` / `queue_note_off` by sending MIDI bytes out the port. Produces silence in the audio buffer.

### 4. Audio engine integration

**File:** `crates/signum-services/src/audio_engine.rs`

For tracks with ExternalMidi instrument:
- Collect MIDI events from clips/keyboard as usual
- Process through MIDI FX chain as usual
- Route to `MidiConnection` instead of internal audio rendering
- Output silence for that track's audio contribution

### 5. UI: MIDI output device selector

**File:** `crates/signum-gui/src/panels/device_rack.rs`

Add "External MIDI" option in device rack:
- Dropdown to select MIDI output port (enumerated from `MidiOutputService::list_ports()`)
- Channel selector (1-16)
- Port refresh button
- Connection status indicator

## MIDI Byte Protocol

```
Note On:       [0x90 | channel, pitch, velocity]
Note Off:      [0x80 | channel, pitch, 0]
CC:            [0xB0 | channel, cc_number, value]
Program Change:[0xC0 | channel, program]
All Notes Off: [0xB0 | channel, 123, 0]
```

## Files Summary

| File | Type | Purpose |
|------|------|---------|
| `signum-services/Cargo.toml` | Modify | Add `midir = "0.10"` |
| `signum-services/src/midi_output.rs` | New | MIDI output service |
| `signum-services/src/lib.rs` | Modify | Export midi_output module |
| `signum-services/src/audio_effects/mod.rs` | Modify | Add ExternalMidi variant |
| `signum-services/src/audio_engine.rs` | Modify | Route to MIDI output |
| `signum-gui/src/panels/device_rack.rs` | Modify | MIDI port selection UI |
| `signum-gui/src/app.rs` | Modify | Wire up MidiOutputService |

## Usage Example

1. Plug in Prophet Rev2 via USB
2. In device rack, click "+ Add Effect" → select "External MIDI"
3. Choose "Prophet Rev2" from port dropdown
4. Set channel to 1
5. Play QWERTY keyboard → Rev2 plays notes
6. MIDI FX chain (Transpose, Arpeggiator, etc.) processes before sending

## Future Enhancements

- MIDI input (record from hardware keyboard)
- MIDI clock sync (send clock to external gear for tempo sync)
- Program change automation
- CC automation lanes
- SysEx support for patch dumps
- Multi-port routing (split keyboard across devices)
