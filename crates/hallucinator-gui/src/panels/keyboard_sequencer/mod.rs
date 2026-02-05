//! QWERTY keyboard sequencer - Factory Rat-style pad grid with scale-aware keyboard

mod drawing;
mod input;
mod types;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use egui::{Key, Ui};
use hallucinator_core::ScaleMode;
use hallucinator_services::EngineState;

pub use types::{DrumStep, KeyboardSequencerAction};
use types::*;

use crate::clipboard::DawClipboard;

/// QWERTY keyboard sequencer panel
pub struct KeyboardSequencerPanel {
    pub(super) drum_steps: Vec<DrumStep>,
    pub(super) pressed_keys: HashMap<Key, u8>,
    pub(super) current_step: usize,
    pub(super) elasticity_pct: f64,
    pub(super) base_velocity: u8,
    pub(super) scale_mode: ScaleMode,
    pub(super) root_note: u8,
    pub is_floating: bool,
    pub(super) selected_step: Option<usize>,
    pub(super) active_drum_layer: usize,
    pub(super) drum_expanded: bool,
}

impl Default for KeyboardSequencerPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardSequencerPanel {
    pub fn new() -> Self {
        Self {
            drum_steps: vec![DrumStep::default(); 12],
            pressed_keys: HashMap::new(),
            current_step: 0,
            elasticity_pct: 0.0,
            base_velocity: 100,
            scale_mode: ScaleMode::Chromatic,
            root_note: 0,
            is_floating: false,
            selected_step: None,
            active_drum_layer: 0,
            drum_expanded: false,
        }
    }

    pub(super) fn layout(&self) -> &'static PadLayout {
        if self.is_floating { &FLOATING } else { &DOCKED }
    }

    pub(super) fn step_count(&self) -> usize {
        let len = self.scale_mode.intervals().len();
        if self.scale_mode == ScaleMode::Chromatic { return len; }
        len + 1
    }

    pub(super) fn pitch_for_step(&self, base_pitch: u8, step: usize) -> u8 {
        let intervals = self.scale_mode.intervals();
        let root = if self.scale_mode == ScaleMode::Chromatic { 0 } else { self.root_note };
        if step >= intervals.len() { return base_pitch + root + 12; }
        base_pitch + root + intervals[step]
    }

    pub(super) fn note_name_for_step(&self, step: usize) -> &'static str {
        let intervals = self.scale_mode.intervals();
        let root = if self.scale_mode == ScaleMode::Chromatic { 0 } else { self.root_note };
        let semitone = if step >= intervals.len() { 0 } else { intervals[step] };
        NOTE_NAMES[(root + semitone) as usize % 12]
    }

    pub(super) fn is_step_accidental(&self, step: usize) -> bool {
        if self.scale_mode != ScaleMode::Chromatic { return false; }
        IS_BLACK_KEY[step % 12]
    }

    fn row_label(&self, base_pitch: u8) -> String {
        let root = if self.scale_mode == ScaleMode::Chromatic { 0 } else { self.root_note };
        let start = base_pitch + root;
        let octave = (start as i32 / 12) - 1;
        format!("{}{}", NOTE_NAMES[start as usize % 12], octave)
    }

    pub fn set_step_sample_name(&mut self, step: usize, layer: usize, name: String) {
        if step < self.drum_steps.len() && layer < 12 {
            self.drum_steps[step].layers[layer].sample_name = Some(name);
            self.drum_steps[step].layers[layer].active = true;
            self.drum_steps[step].active = true;
        }
    }

    pub fn clear_step_sample_name(&mut self, step: usize, layer: usize) {
        if step < self.drum_steps.len() && layer < 12 {
            self.drum_steps[step].layers[layer].sample_name = None;
            self.drum_steps[step].layers[layer].active = false;
        }
    }

    /// Sync panel's drum pattern to engine state for sample-accurate playback
    pub fn sync_pattern_to_engine(&self, engine_state: &Arc<EngineState>, instrument_id: Option<u64>) {
        let Ok(mut pattern) = engine_state.drum_pattern.lock() else { return };
        pattern.step_count = self.step_count();
        pattern.instrument_id = instrument_id;
        for (i, step) in self.drum_steps.iter().enumerate() {
            if i >= 12 { break; }
            pattern.steps[i].active = step.active;
            pattern.steps[i].active_layers = step.active_layer_mask();
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        track_name: Option<&str>,
        is_playing: bool,
        clipboard: &DawClipboard,
        engine_state: &Arc<EngineState>,
    ) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let sc = self.step_count();

        // Read current step from audio thread (sample-accurate timing)
        self.current_step = engine_state.drum_current_step.load(Ordering::Relaxed) % sc;

        // Dark panel background
        let panel_rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(panel_rect, 0.0, PANEL_BG);

        // Compact toolbar
        self.draw_toolbar(ui, track_name);

        // Resize drum steps if scale changed
        self.drum_steps.resize(sc, DrumStep::default());
        if self.selected_step.is_some_and(|s| s >= sc) {
            self.selected_step = None;
        }
        ui.add_space(2.0);

        // Input handling
        actions.extend(self.handle_drum_input(ui));
        actions.extend(self.handle_melodic_input(ui));
        actions.extend(self.handle_drum_copy_paste(ui, clipboard));

        // Pad grid
        let l = self.layout();
        let sp = l.spacing;
        let label_3 = self.row_label(48);
        let label_4 = self.row_label(60);
        let label_5 = self.row_label(72);
        ui.vertical(|ui| {
            if self.drum_expanded {
                actions.extend(self.draw_expanded_grid(ui, is_playing, clipboard));
                ui.add_space(sp);
            }
            actions.extend(self.draw_drum_row(ui, is_playing, clipboard));
            ui.add_space(sp);
            self.draw_melodic_row(ui, &label_3, &OCTAVE_3_KEYS[..sc], 48, is_playing);
            ui.add_space(sp);
            self.draw_melodic_row(ui, &label_4, &OCTAVE_4_KEYS[..sc], 60, is_playing);
            ui.add_space(sp);
            self.draw_melodic_row(ui, &label_5, &OCTAVE_5_KEYS[..sc], 72, is_playing);
        });

        actions
    }

    fn draw_toolbar(&mut self, ui: &mut Ui, track_name: Option<&str>) {
        ui.horizontal(|ui| {
            ui.colored_label(LABEL_BRIGHT, "FACTORY RAT");
            if let Some(name) = track_name {
                ui.colored_label(LABEL_DIM, name);
            }
            let float_label = if self.is_floating { "Dock" } else { "Float" };
            if ui.button(float_label).clicked() {
                self.is_floating = !self.is_floating;
            }
            ui.separator();
            egui::ComboBox::from_id_salt("scale_combo")
                .selected_text(self.scale_mode.name())
                .width(90.0)
                .show_ui(ui, |ui| {
                    for &mode in &ALL_SCALES {
                        ui.selectable_value(&mut self.scale_mode, mode, mode.name());
                    }
                });
            if self.scale_mode != ScaleMode::Chromatic {
                egui::ComboBox::from_id_salt("root_combo")
                    .selected_text(NOTE_NAMES[self.root_note as usize % 12])
                    .width(40.0)
                    .show_ui(ui, |ui| {
                        for (i, &name) in NOTE_NAMES.iter().enumerate() {
                            ui.selectable_value(&mut self.root_note, i as u8, name);
                        }
                    });
            }
            ui.separator();
            ui.colored_label(LABEL_DIM, "Ph");
            ui.add(egui::Slider::new(&mut self.elasticity_pct, -10.0..=10.0)
                .suffix("%")
                .fixed_decimals(1));
            ui.colored_label(LABEL_DIM, "Vel");
            let mut vel = self.base_velocity as f32;
            ui.add(egui::Slider::new(&mut vel, 1.0..=127.0).fixed_decimals(0));
            self.base_velocity = vel as u8;

            ui.separator();
            ui.colored_label(LABEL_DIM, "Lyr");
            egui::ComboBox::from_id_salt("dr_layer")
                .selected_text(format!("{}", self.active_drum_layer + 1))
                .width(32.0)
                .show_ui(ui, |ui| {
                    for layer_idx in 0..12 {
                        ui.selectable_value(
                            &mut self.active_drum_layer,
                            layer_idx,
                            format!("{}", layer_idx + 1),
                        );
                    }
                });
            let expand_icon = if self.drum_expanded { "\u{25B2}" } else { "\u{25BC}" };
            if ui.small_button(expand_icon).clicked() {
                self.drum_expanded = !self.drum_expanded;
            }
        });
    }
}
