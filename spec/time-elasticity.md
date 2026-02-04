# Time Elasticity + Pseudo-Velocity for Keyboard Sequencer

## Feature 1: Time Elasticity (Track Phasing)

Percentage-based BPM drift per track. At 0% a track follows the master clock exactly. At +2% it runs 2% faster, gradually phasing ahead (Steve Reich-style). Negative values phase behind.

### How It Works

```
effective_beat = master_beat * (1.0 + elasticity_pct / 100.0)
current_step = (effective_beat as usize) % 12
```

At +1% elasticity over 100 beats, the track gains 1 beat — shifting through all 12 steps relative to other tracks.

### Implementation

**File: `crates/signum-gui/src/panels/keyboard_sequencer.rs`**

Add to struct:
```rust
pub struct KeyboardSequencerPanel {
    // ... existing fields ...
    elasticity_pct: f64,  // -10.0 to +10.0 (percentage BPM drift)
}
```

In `ui()`, modify step calculation:
```rust
let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
let master_beat = playback_position as f64 / samples_per_beat;
let elastic_beat = master_beat * (1.0 + self.elasticity_pct / 100.0);
self.current_step = (elastic_beat as usize) % 12;
```

Add UI control — a small slider in the header:
```rust
ui.horizontal(|ui| {
    ui.heading("Keyboard");
    // ...
    ui.label("Phase");
    ui.add(egui::Slider::new(&mut self.elasticity_pct, -10.0..=10.0)
        .suffix("%")
        .fixed_decimals(1));
});
```

---

## Feature 2: Pseudo-Velocity

Velocity control via modifier keys + a velocity slider for fine tuning.

### Scheme

| Input | Velocity |
|-------|----------|
| Key alone | Use slider value (default 100) |
| Shift + Key | 127 (accent) |
| Ctrl + Key | 40 (ghost/soft) |

### Implementation

**File: `crates/signum-gui/src/panels/keyboard_sequencer.rs`**

Add to struct:
```rust
pub struct KeyboardSequencerPanel {
    // ... existing fields ...
    base_velocity: u8,  // 1-127, default 100
}
```

In `handle_melodic_input()`, resolve velocity from modifiers:
```rust
let velocity = if ui.input(|i| i.modifiers.shift) {
    127
} else if ui.input(|i| i.modifiers.ctrl) {
    40
} else {
    self.base_velocity
};
// ... use velocity in PlayNote action
```

Add slider in header:
```rust
ui.label("Vel");
let mut vel = self.base_velocity as f32;
ui.add(egui::Slider::new(&mut vel, 1.0..=127.0).fixed_decimals(0));
self.base_velocity = vel as u8;
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/signum-gui/src/panels/keyboard_sequencer.rs` | Add elasticity_pct, base_velocity fields; modify step calc; add UI sliders; modify velocity in PlayNote |

Single file change.

## Verification

1. Set elasticity to +5%, press play → steps advance faster than master BPM
2. Set elasticity to -5% → steps lag behind
3. Set elasticity to 0% → exact sync with master
4. Press Shift+Q → note plays at velocity 127
5. Press Ctrl+Q → note plays at velocity 40
6. Drag velocity slider to 80, press Q → note plays at velocity 80
7. Velocity slider visible in header next to "Phase" slider
