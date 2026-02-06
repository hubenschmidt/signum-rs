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
use types::{
    SelectionState, SequencerRow, RepeatRate, PadLayout, FLOATING, DOCKED,
    NOTE_NAMES, IS_BLACK_KEY, ALL_SCALES, ALL_REPEAT_RATES,
    PANEL_BG, LABEL_BRIGHT, LABEL_DIM,
    OCTAVE_3_KEYS, OCTAVE_4_KEYS, OCTAVE_5_KEYS,
};

use crate::clipboard::DawClipboard;

/// QWERTY keyboard sequencer panel
pub struct KeyboardSequencerPanel {
    pub(super) drum_steps: Vec<DrumStep>,
    pub(super) pressed_keys: HashMap<Key, u8>,
    pub(super) current_step: usize,
    pub(super) base_velocity: u8,
    pub(super) scale_mode: ScaleMode,
    pub(super) root_note: u8,
    pub is_floating: bool,
    pub(super) sel: SelectionState,
    pub(super) drum_expanded: bool,
    /// Bitmask of drum steps currently being triggered (keys held down)
    pub(super) triggered_steps: u16,
    /// Number of drum steps (independent from scale mode): 4, 6, 8, or 12
    pub(super) drum_step_count: usize,
    /// Loop length in bars: 1, 2, or 4
    pub(super) drum_loop_bars: u8,
    /// Whether to snap pattern length to arrange view loop region
    pub(super) snap_to_arrange: bool,
    /// Note repeat rate for MPC-style rolls
    pub(super) repeat_rate: RepeatRate,
    /// Last repeat trigger time per step (in beats)
    pub(super) last_repeat_beat: HashMap<usize, f64>,
    /// Sample assigned to each row (one sample per row, shared across all steps)
    pub(super) row_samples: [Option<String>; 12],
    /// Whether each row is enabled (unmuted) - true = plays, false = muted
    pub(super) row_enabled: [bool; 12],
    /// Pending Tab press from app level (Some(true) = shift+tab, Some(false) = tab, None = no tab)
    pub(super) pending_tab: Option<bool>,
}

impl Default for KeyboardSequencerPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardSequencerPanel {
    pub fn new() -> Self {
        Self {
            drum_steps: vec![DrumStep::default(); 8],
            pressed_keys: HashMap::new(),
            current_step: 0,
            base_velocity: 100,
            scale_mode: ScaleMode::Chromatic,
            root_note: 0,
            is_floating: false,
            sel: SelectionState::default(),
            drum_expanded: true,
            triggered_steps: 0,
            drum_step_count: 8,
            drum_loop_bars: 1,
            snap_to_arrange: false,
            repeat_rate: RepeatRate::default(),
            last_repeat_beat: HashMap::new(),
            row_samples: std::array::from_fn(|_| None),
            row_enabled: [true; 12],  // All rows enabled by default
            pending_tab: None,
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

    /// Set sample for an entire row (layer)
    pub fn set_row_sample(&mut self, row: usize, name: String) {
        if row < 12 {
            self.row_samples[row] = Some(name);
        }
    }

    /// Clear sample from a row
    pub fn clear_row_sample(&mut self, row: usize) {
        if row < 12 {
            self.row_samples[row] = None;
        }
    }

    /// Check if a row is enabled (unmuted)
    pub fn is_row_enabled(&self, row: usize) -> bool {
        row < 12 && self.row_enabled[row]
    }

    /// Set pending Tab press from app level
    pub fn set_pending_tab(&mut self, shift: bool) {
        self.pending_tab = Some(shift);
    }

    /// Sync panel's drum pattern to engine state for sample-accurate playback
    pub fn sync_pattern_to_engine(&self, engine_state: &Arc<EngineState>, instrument_id: Option<u64>) {
        let Ok(mut pattern) = engine_state.drum_pattern.lock() else { return };
        pattern.step_count = self.drum_step_count;
        pattern.loop_bars = self.drum_loop_bars;
        pattern.snap_to_arrange = self.snap_to_arrange;
        pattern.instrument_id = instrument_id;
        // Convert row_enabled array to bitmask
        pattern.row_enabled = self.row_enabled.iter().enumerate()
            .filter(|(_, enabled)| **enabled)
            .fold(0u16, |mask, (i, _)| mask | (1 << i));
        for (i, step) in self.drum_steps.iter().enumerate().take(12) {
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
        let sc = self.step_count();  // for melodic rows (scale-based)
        let dsc = self.drum_step_count;  // for drum rows (independent)

        // Read current step from audio thread (sample-accurate timing)
        self.current_step = engine_state.drum_current_step.load(Ordering::Relaxed) % dsc;

        // Dark panel background
        let panel_rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(panel_rect, 0.0, PANEL_BG);

        // Compact toolbar
        self.draw_toolbar(ui, track_name);

        // Resize drum steps based on drum_step_count (independent from scale)
        self.drum_steps.resize(dsc, DrumStep::default());
        if self.sel.selected_step.is_some_and(|s| s >= dsc) {
            self.sel.selected_step = None;
        }
        ui.add_space(2.0);

        // Input handling (Tab consumed at app level, stored in pending_tab)
        self.handle_row_navigation();
        self.handle_arrow_navigation(ui);
        actions.extend(self.handle_drum_input(ui, engine_state));
        actions.extend(self.handle_melodic_input(ui));
        actions.extend(self.handle_drum_copy_paste(ui, clipboard));

        // Pad grid
        let l = self.layout();
        let sp = l.spacing;
        let label_3 = self.row_label(48);
        let label_4 = self.row_label(60);
        let label_5 = self.row_label(72);
        let active_row = self.sel.active_row;
        let mut interactions = Vec::new();
        ui.vertical(|ui| {
            if self.drum_expanded {
                let (acts, ints) = self.draw_expanded_grid(ui, is_playing, clipboard);
                actions.extend(acts);
                interactions.extend(ints);
                ui.add_space(sp);
            }
            let (acts, ints) = self.draw_drum_row(ui, is_playing, clipboard, active_row == SequencerRow::Drum);
            actions.extend(acts);
            interactions.extend(ints);
            ui.add_space(sp);
            interactions.extend(self.draw_melodic_row(ui, &label_3, &OCTAVE_3_KEYS[..sc], 48, is_playing, active_row == SequencerRow::Octave3, SequencerRow::Octave3));
            ui.add_space(sp);
            interactions.extend(self.draw_melodic_row(ui, &label_4, &OCTAVE_4_KEYS[..sc], 60, is_playing, active_row == SequencerRow::Octave4, SequencerRow::Octave4));
            ui.add_space(sp);
            interactions.extend(self.draw_melodic_row(ui, &label_5, &OCTAVE_5_KEYS[..sc], 72, is_playing, active_row == SequencerRow::Octave5, SequencerRow::Octave5));
        });
        actions.extend(self.handle_grid_interactions(interactions));

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
            ui.colored_label(LABEL_DIM, "Vel");
            let mut vel = self.base_velocity as f32;
            ui.add(egui::Slider::new(&mut vel, 1.0..=127.0).fixed_decimals(0));
            self.base_velocity = vel as u8;

            ui.separator();
            ui.colored_label(LABEL_DIM, "Lyr");
            egui::ComboBox::from_id_salt("dr_layer")
                .selected_text(format!("{}", self.sel.active_drum_layer + 1))
                .width(32.0)
                .show_ui(ui, |ui| {
                    for layer_idx in 0..12 {
                        ui.selectable_value(
                            &mut self.sel.active_drum_layer,
                            layer_idx,
                            format!("{}", layer_idx + 1),
                        );
                    }
                });
            let expand_icon = if self.drum_expanded { "\u{25B2}" } else { "\u{25BC}" };
            if ui.small_button(expand_icon).clicked() {
                self.drum_expanded = !self.drum_expanded;
            }

            ui.separator();
            ui.colored_label(LABEL_DIM, "Steps");
            egui::ComboBox::from_id_salt("drum_steps")
                .selected_text(format!("{}", self.drum_step_count))
                .width(36.0)
                .show_ui(ui, |ui| {
                    for &count in &[4usize, 6, 8, 12] {
                        ui.selectable_value(&mut self.drum_step_count, count, format!("{}", count));
                    }
                });

            ui.colored_label(LABEL_DIM, "Bars");
            egui::ComboBox::from_id_salt("loop_bars")
                .selected_text(format!("{}", self.drum_loop_bars))
                .width(32.0)
                .show_ui(ui, |ui| {
                    for &bars in &[1u8, 2, 4] {
                        ui.selectable_value(&mut self.drum_loop_bars, bars, format!("{}", bars));
                    }
                });

            let snap_label = if self.snap_to_arrange { "Snap" } else { "Free" };
            if ui.small_button(snap_label).on_hover_text("Snap to arrange loop").clicked() {
                self.snap_to_arrange = !self.snap_to_arrange;
            }

            ui.separator();
            ui.colored_label(LABEL_DIM, "Rpt");
            egui::ComboBox::from_id_salt("repeat_rate")
                .selected_text(self.repeat_rate.name())
                .width(40.0)
                .show_ui(ui, |ui| {
                    for &rate in &ALL_REPEAT_RATES {
                        ui.selectable_value(&mut self.repeat_rate, rate, rate.name());
                    }
                });
        });
    }
}
