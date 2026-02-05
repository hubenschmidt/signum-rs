//! Input handling for the keyboard sequencer panel.

use std::sync::Arc;

use egui::{Key, Ui};

use crate::clipboard::{ClipboardContent, DawClipboard};

use super::types::*;
use super::KeyboardSequencerPanel;

impl KeyboardSequencerPanel {
    pub(super) fn handle_drum_input(&mut self, ui: &mut Ui) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let sc = self.step_count();

        for (i, &key) in DRUM_KEYS[..sc].iter().enumerate() {
            if !ui.input(|inp| inp.key_pressed(key)) { continue; }
            self.drum_steps[i].active = !self.drum_steps[i].active;
            actions.push(KeyboardSequencerAction::ToggleDrumStep(i));
            // Preview sound when toggling ON a step that has any active sample
            let mask = self.drum_steps[i].active_layer_mask();
            if self.drum_steps[i].active && mask != 0 {
                actions.push(KeyboardSequencerAction::PlayDrumStep { step: i, velocity: self.base_velocity, active_layers: mask });
            }
        }

        actions
    }

    pub(super) fn handle_drum_copy_paste(
        &mut self,
        ui: &mut Ui,
        clipboard: &DawClipboard,
    ) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let modifiers = ui.input(|i| i.modifiers);
        let ctrl = modifiers.ctrl || modifiers.mac_cmd;

        // Ctrl+C: copy selected step's active layer
        if ctrl && ui.input(|i| i.key_pressed(Key::C)) {
            if let Some(sel) = self.selected_step {
                if self.drum_steps[sel].layers[self.active_drum_layer].sample_name.is_some() {
                    actions.push(KeyboardSequencerAction::CopyDrumStep { step: sel, layer: self.active_drum_layer });
                }
            }
        }

        // Ctrl+V: paste from clipboard — check Event::Paste (platform Ctrl+V)
        // and raw key as fallback
        let paste = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))))
            || (ctrl && ui.input(|i| i.key_pressed(Key::V)));
        if paste {
            if let Some(to) = self.selected_step {
                actions.extend(self.paste_from_clipboard(clipboard, to, self.active_drum_layer));
            }
        }

        // Delete/Backspace: clear selected step's active layer sample
        let delete = ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace));
        let step = self.selected_step;
        let layer = self.active_drum_layer;
        let has_sample = step.map(|s| self.drum_steps[s].layers[layer].sample_name.is_some()).unwrap_or(false);

        if delete && has_sample {
            let step = step.unwrap();
            self.drum_steps[step].layers[layer].sample_name = None;
            self.drum_steps[step].layers[layer].active = false;
            actions.push(KeyboardSequencerAction::ClearStepSample { step, layer });
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
