# Sample-Accurate Sequencer Timing

## Problem

The drum sequencer currently triggers samples from the GUI thread (~60fps), causing up to 16ms timing jitter. At 120 BPM, a 16th note is ~125ms, so 16ms jitter is audibly "sloppy."

**Current flow (broken):**
1. Audio thread updates `position` atomically
2. GUI thread reads `position`, calculates step, triggers sample
3. Audio thread processes triggered voice on next callback

The variable delay between steps 2-3 causes rhythmic inconsistency.

## Solution

Move step tracking and sample triggering to the audio thread for sample-accurate timing.

**New flow:**
1. GUI edits pattern data (which steps active, which samples loaded)
2. Audio thread reads pattern, calculates step at exact sample boundaries, triggers directly
3. Zero timing jitter

## Data Model

### New: `DrumPattern` in `EngineState`

```rust
// In audio_engine.rs or new file

#[derive(Clone, Default)]
pub struct DrumPatternStep {
    pub active: bool,
    pub active_layers: u16,  // bitmask of which layers fire
}

pub struct DrumPattern {
    pub steps: [DrumPatternStep; 12],
    pub step_count: usize,           // 8 or 12 typically
    pub instrument_id: Option<u64>,  // which SampleKit to trigger
}

// In EngineState
pub struct EngineState {
    // ... existing fields ...
    pub drum_pattern: Mutex<DrumPattern>,
    pub drum_current_step: AtomicUsize,  // for GUI display only
}
```

### GUI Panel Changes

`KeyboardSequencerPanel` no longer triggers samples. It:
1. Updates `drum_pattern` when user toggles steps or loads samples
2. Reads `drum_current_step` for visual highlighting

```rust
// In keyboard_sequencer/mod.rs - remove triggering logic
if is_playing && sample_rate > 0 {
    // Only update visual, don't trigger
    let steps_per_beat = sc as f64 / 4.0;
    let new_step = ((elastic_beat * steps_per_beat) as usize) % sc;
    self.current_step = new_step;  // visual only
}
```

## Audio Thread Integration

### In `render_audio()`

Add step tracking between instrument processing and buffer fill:

```rust
fn render_audio(state: &EngineState, buffer: &mut [f32], channels: u16) {
    // ... existing instrument lock and MIDI processing ...

    // Drum sequencer - sample-accurate step triggering
    if is_playing {
        let pattern = state.drum_pattern.lock().ok();
        if let Some(pattern) = pattern {
            if let Some(inst_id) = pattern.instrument_id {
                if let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&inst_id) {
                    let samples_per_step = (sample_rate as f64 * 60.0 / bpm) * 4.0 / pattern.step_count as f64;

                    for frame_idx in 0..num_frames {
                        let abs_pos = pos + frame_idx as u64;
                        let step = ((abs_pos as f64 / samples_per_step) as usize) % pattern.step_count;
                        let prev_step = state.drum_current_step.load(Ordering::Relaxed);

                        if step != prev_step {
                            state.drum_current_step.store(step, Ordering::Relaxed);
                            let step_data = &pattern.steps[step];
                            if step_data.active && step_data.active_layers != 0 {
                                kit.trigger_step(step, 100, step_data.active_layers);
                            }
                        }
                    }
                }
            }
        }
    }

    // ... rest of render_audio ...
}
```

## Synchronization Points

| Component | Reads | Writes |
|-----------|-------|--------|
| GUI thread | `drum_current_step` (atomic) | `drum_pattern` (mutex) |
| Audio thread | `drum_pattern` (mutex) | `drum_current_step` (atomic) |

Pattern updates from GUI are infrequent (user edits), so mutex contention is minimal.

## Files to Modify

| File | Change |
|------|--------|
| `audio_engine.rs` | Add `DrumPattern`, `DrumPatternStep` structs. Add `drum_pattern: Mutex<DrumPattern>` and `drum_current_step: AtomicUsize` to `EngineState`. Add step tracking in `render_audio()`. |
| `keyboard_sequencer/mod.rs` | Remove `PlayDrumStep` action generation. Read `drum_current_step` for visual. Update pattern via engine state instead of direct triggering. |
| `keyboard_sequencer/types.rs` | Remove `PlayDrumStep` from action enum (or keep for manual preview). |
| `app/action_handlers.rs` | Update handlers to modify `drum_pattern` instead of triggering directly. |
| `app/mod.rs` | Pass `engine_state` to sequencer panel for pattern sync. |

## Migration Steps

1. Add `DrumPattern` and `DrumPatternStep` to `audio_engine.rs`
2. Add fields to `EngineState::new()`
3. Add step tracking loop in `render_audio()`
4. Update `KeyboardSequencerPanel` to sync pattern to engine state
5. Remove GUI-based triggering from sequencer
6. Update action handlers

## Verification

1. `cargo check` passes
2. Load kick sample, activate step 1
3. Play 4-bar loop - kicks should be tight on the beat
4. Compare to metronome - no audible drift or jitter
5. Rapidly toggle steps while playing - no glitches
6. Stop/start playback - step position resets correctly

## Edge Cases

- **Loop boundary**: When loop wraps, step resets to correct position
- **Tempo change**: `samples_per_step` recalculates automatically
- **Pattern edit during playback**: Mutex ensures atomic update
- **No instrument assigned**: Skip triggering gracefully
