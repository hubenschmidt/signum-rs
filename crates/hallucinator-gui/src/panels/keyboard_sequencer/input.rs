//! Input handling for the keyboard sequencer panel.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use egui::{Key, Ui};
use hallucinator_services::EngineState;

use crate::clipboard::{ClipboardContent, DawClipboard};

use super::KeyboardSequencerPanel;
use super::types::{
    GridInteraction, KeyboardSequencerAction, RepeatRate, SequencerRow,
    DRUM_KEYS, OCTAVE_3_KEYS, OCTAVE_4_KEYS, OCTAVE_5_KEYS,
};

impl KeyboardSequencerPanel {
    /// Handle Tab/Shift+Tab to cycle through sequencer rows
    pub(super) fn handle_row_navigation(&mut self) {
        // Tab is consumed at app level and stored in pending_tab
        let Some(shift_held) = self.pending_tab.take() else {
            return;
        };

        self.sel.active_row = if shift_held {
            self.sel.active_row.prev(self.drum_expanded)
        } else {
            self.sel.active_row.next(self.drum_expanded)
        };

        // Sync active_drum_layer when navigating to a drum layer row
        if let Some(layer) = self.sel.active_row.drum_layer() {
            self.sel.active_drum_layer = layer;
        }
    }

    /// Handle arrow key navigation when a step is selected
    /// Arrows navigate the selected cell in any direction
    pub(super) fn handle_arrow_navigation(&mut self, ui: &mut Ui) {
        let Some(current_step) = self.sel.selected_step else { return };

        // Consume arrow keys to prevent other widgets from processing them
        let left = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft));
        let right = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowRight));
        let up = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp));
        let down = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown));

        let max_step = self.drum_step_count.saturating_sub(1);

        // Left/Right: navigate steps
        self.sel.selected_step = left.then(|| current_step.saturating_sub(1)).or(self.sel.selected_step);
        self.sel.selected_step = right.then(|| (current_step + 1).min(max_step)).or(self.sel.selected_step);

        // Up/Down: navigate rows (move selected cell to different row)
        let new_row = match (up, down) {
            (true, _) => Some(self.sel.active_row.prev(self.drum_expanded)),
            (_, true) => Some(self.sel.active_row.next(self.drum_expanded)),
            _ => None,
        };
        if let Some(row) = new_row {
            self.sel.active_row = row;
            self.sel.active_drum_layer = row.drum_layer().unwrap_or(self.sel.active_drum_layer);
        }
    }

    pub(super) fn handle_drum_input(&mut self, ui: &mut Ui, engine_state: &Arc<EngineState>) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let has_ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
        let dsc = self.drum_step_count;

        // Get current beat for note repeat timing
        let current_beat = self.get_current_beat(engine_state);

        // Determine active row for triggering
        let active_row = match self.sel.active_row {
            SequencerRow::DrumLayer(layer) => Some(layer),
            SequencerRow::Drum => Some(self.sel.active_drum_layer),
            _ => None,
        };

        for (i, &key) in DRUM_KEYS[..dsc].iter().enumerate() {
            let is_down = ui.input(|inp| inp.key_down(key));
            let was_down = (self.triggered_steps & (1 << i)) != 0;

            // Guard: key released — clear state
            if !is_down && was_down {
                self.triggered_steps &= !(1 << i);
                self.last_repeat_beat.remove(&i);
            }

            // Guard: key not pressed or not on a drum row
            let Some(row) = active_row.filter(|_| is_down) else { continue };

            // New press: trigger sample and optionally toggle step
            if !was_down {
                self.triggered_steps |= 1 << i;
                self.last_repeat_beat.insert(i, current_beat);

                if has_ctrl {
                    self.drum_steps[i].layers[row].active = !self.drum_steps[i].layers[row].active;
                    actions.push(KeyboardSequencerAction::ToggleDrumStep(i));
                }
                if self.row_samples[row].is_some() {
                    actions.push(KeyboardSequencerAction::PlayRowSample { row, velocity: self.base_velocity });
                }
                continue;
            }

            // Held: note repeat (guard: repeat off or interval not elapsed)
            let Some(interval) = self.repeat_rate.beats() else { continue };
            let last = self.last_repeat_beat.get(&i).copied().unwrap_or(0.0);
            if current_beat < last + interval { continue; }

            self.last_repeat_beat.insert(i, current_beat);
            if self.row_samples[row].is_some() {
                actions.push(KeyboardSequencerAction::PlayRowSample { row, velocity: self.base_velocity });
            }
        }

        actions
    }

    /// Toggle drum step/layer active state based on current row
    fn handle_drum_toggle(&mut self, step: usize) {
        if let SequencerRow::DrumLayer(layer) = self.sel.active_row {
            self.drum_steps[step].layers[layer].active = !self.drum_steps[step].layers[layer].active;
            self.drum_steps[step].active = self.drum_steps[step].active_layer_mask() != 0;
            return;
        }
        if self.sel.active_row == SequencerRow::Drum {
            self.drum_steps[step].active = !self.drum_steps[step].active;
        }
    }

    /// Get current playback position in beats
    fn get_current_beat(&self, engine_state: &Arc<EngineState>) -> f64 {
        let position = engine_state.position.load(Ordering::Relaxed);
        let Ok(timeline) = engine_state.timeline.lock() else { return 0.0 };
        let sample_rate = timeline.transport.sample_rate as f64;
        let bpm = timeline.transport.bpm;
        if sample_rate == 0.0 { return 0.0; }
        position as f64 / sample_rate * bpm / 60.0
    }

    pub(super) fn handle_drum_copy_paste(
        &mut self,
        ui: &mut Ui,
        clipboard: &DawClipboard,
    ) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let modifiers = ui.input(|i| i.modifiers);
        let ctrl = modifiers.ctrl || modifiers.mac_cmd;

        // Get active row for row-level operations
        let active_row = match self.sel.active_row {
            SequencerRow::DrumLayer(row) => Some(row),
            _ => None,
        };

        // Ctrl+C: copy row sample
        let c_pressed = ui.input(|i| {
            i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::C, pressed: true, .. }))
        });
        let copy_row = active_row.filter(|&r| ctrl && c_pressed && self.row_samples[r].is_some());
        if let Some(row) = copy_row {
            actions.push(KeyboardSequencerAction::CopyRowSample { row });
        }

        // Ctrl+V: paste to row
        let v_pressed = ui.input(|i| {
            i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::V, pressed: true, .. }))
        });
        let paste_event = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))));
        let paste = paste_event || (ctrl && v_pressed);
        let paste_row = active_row.filter(|_| paste && clipboard.content().is_some());
        if let Some(row) = paste_row {
            actions.push(KeyboardSequencerAction::PasteRowSample { row });
        }

        // Delete/Backspace: clear row sample
        let delete = ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace));
        let delete_row = active_row.filter(|&r| delete && self.row_samples[r].is_some());
        if let Some(row) = delete_row {
            self.row_samples[row] = None;
            actions.push(KeyboardSequencerAction::ClearRowSample { row });
        }

        actions
    }

    pub(super) fn paste_from_clipboard(
        &self,
        clipboard: &DawClipboard,
        to_step: usize,
        to_layer: usize,
    ) -> Vec<KeyboardSequencerAction> {
        let Some(content) = clipboard.content() else { return Vec::new() };
        match content {
            ClipboardContent::FilePath(path) => {
                vec![KeyboardSequencerAction::LoadStepSample { step: to_step, layer: to_layer, path: path.clone() }]
            }
            ClipboardContent::SampleData { name, data } => {
                vec![KeyboardSequencerAction::PasteStepSample {
                    step: to_step,
                    layer: to_layer,
                    name: name.clone(),
                    data: Arc::clone(data),
                }]
            }
        }
    }

    /// Process grid interactions detected during drawing (SoC: mutations live here, not in drawing)
    pub(super) fn handle_grid_interactions(&mut self, interactions: Vec<GridInteraction>) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        for interaction in interactions {
            match interaction {
                GridInteraction::DrumRowClick { step } => {
                    self.sel.selected_step = Some(step);
                    self.sel.active_row = SequencerRow::Drum;
                    self.drum_steps[step].active = !self.drum_steps[step].active;
                    actions.push(KeyboardSequencerAction::ToggleDrumStep(step));
                }
                GridInteraction::SampleButtonClick { row, ctrl } => {
                    self.sel.active_drum_layer = row;
                    self.sel.active_row = SequencerRow::DrumLayer(row);
                    if ctrl {
                        if !self.sel.selected_rows.remove(&row) {
                            self.sel.selected_rows.insert(row);
                        }
                    } else {
                        actions.extend(self.toggle_row_mute_batch(row));
                    }
                }
                GridInteraction::GridCellClick { step, row, ctrl } => {
                    self.sel.selected_step = Some(step);
                    self.sel.active_drum_layer = row;
                    self.sel.active_row = SequencerRow::DrumLayer(row);
                    if ctrl {
                        let cell_key = (row, step);
                        if !self.sel.selected_cells.remove(&cell_key) {
                            self.sel.selected_cells.insert(cell_key);
                        }
                    } else {
                        actions.extend(self.toggle_grid_cell_batch(step, row));
                    }
                }
                GridInteraction::MelodicCellClick { step, row } => {
                    self.sel.selected_step = Some(step);
                    self.sel.active_row = row;
                }
            }
        }
        actions
    }

    /// Toggle mute for a row and propagate to all selected rows
    pub(super) fn toggle_row_mute_batch(&mut self, row: usize) -> Vec<KeyboardSequencerAction> {
        if self.row_samples[row].is_none() { return Vec::new(); }

        let mut actions = Vec::new();
        let new_enabled = !self.row_enabled[row];
        self.row_enabled[row] = new_enabled;
        actions.push(KeyboardSequencerAction::ToggleRowEnabled { row });

        let sel_rows: Vec<_> = self.sel.selected_rows.iter().copied()
            .filter(|&r| r != row && self.row_samples[r].is_some())
            .collect();
        for sel_row in sel_rows {
            self.row_enabled[sel_row] = new_enabled;
            actions.push(KeyboardSequencerAction::ToggleRowEnabled { row: sel_row });
        }
        self.sel.selected_rows.clear();
        actions
    }

    /// Toggle a grid cell's active state and propagate to all selected cells
    pub(super) fn toggle_grid_cell_batch(&mut self, step: usize, row: usize) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let new_active = !self.drum_steps[step].layers[row].active;
        self.drum_steps[step].layers[row].active = new_active;
        actions.push(KeyboardSequencerAction::ToggleDrumStep(step));

        let sel_cells: Vec<_> = self.sel.selected_cells.iter().copied()
            .filter(|&c| c != (row, step))
            .collect();
        for (sel_row, sel_step) in sel_cells {
            self.drum_steps[sel_step].layers[sel_row].active = new_active;
            actions.push(KeyboardSequencerAction::ToggleDrumStep(sel_step));
        }
        self.sel.selected_cells.clear();
        actions
    }

    pub(super) fn handle_melodic_input(&mut self, ui: &mut Ui) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let sc = self.step_count();

        // Skip melodic input when Ctrl/Cmd is held — those combos are app shortcuts (Ctrl+C/V/X)
        let has_shortcut_mod = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
        if has_shortcut_mod {
            return actions;
        }

        let velocity = ui.input(|i| {
            if i.modifiers.shift { return 127u8; }
            self.base_velocity
        });

        let octave_rows: [(&[Key], u8); 3] = [
            (&OCTAVE_3_KEYS[..sc], 48),
            (&OCTAVE_4_KEYS[..sc], 60),
            (&OCTAVE_5_KEYS[..sc], 72),
        ];

        for (keys, base_pitch) in octave_rows {
            for (i, &key) in keys.iter().enumerate() {
                let pitch = self.pitch_for_step(base_pitch, i);
                let is_pressed = ui.input(|inp| inp.key_down(key));
                let was_pressed = self.pressed_keys.contains_key(&key);

                if is_pressed && !was_pressed {
                    self.pressed_keys.insert(key, pitch);
                    actions.push(KeyboardSequencerAction::PlayNote { pitch, velocity });
                    continue;
                }
                if !is_pressed && was_pressed {
                    let sent_pitch = self.pressed_keys.remove(&key).unwrap_or(pitch);
                    actions.push(KeyboardSequencerAction::StopNote { pitch: sent_pitch });
                }
            }
        }

        actions
    }
}
