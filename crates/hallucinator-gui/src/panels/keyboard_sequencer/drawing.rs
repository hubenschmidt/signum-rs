//! Drawing methods for the keyboard sequencer panel.

use std::path::PathBuf;

use egui::{Color32, Key, Rect, Sense, Stroke, Ui, Vec2};

use crate::clipboard::DawClipboard;

use super::types::*;
use super::KeyboardSequencerPanel;

impl KeyboardSequencerPanel {
    pub(super) fn draw_drum_row(
        &mut self,
        ui: &mut Ui,
        is_playing: bool,
        clipboard: &DawClipboard,
        is_active_row: bool,
    ) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let l = self.layout();
        let layer = self.active_drum_layer;
        let sample_btn_w = l.label_w * 1.8;

        ui.horizontal(|ui| {
            // Empty spacer matching sample button width
            ui.allocate_space(Vec2::new(sample_btn_w, l.size));
            ui.add_space(2.0);
            // DR label matching row number width
            let (_, label_painter) = ui.allocate_painter(Vec2::new(l.label_w * 0.6, l.size), Sense::hover());
            let label_color = if is_active_row { LABEL_BRIGHT } else { LABEL_DIM };
            label_painter.text(
                label_painter.clip_rect().center(),
                egui::Align2::CENTER_CENTER,
                "DR",
                egui::FontId::proportional(l.font_pad),
                label_color,
            );

            for i in 0..self.drum_steps.len() {
                let active = self.drum_steps[i].active;
                let has_sample = self.drum_steps[i].layers[layer].sample_name.is_some();
                let is_current = is_playing && self.current_step == i;
                let is_triggered = (self.triggered_steps & (1 << i)) != 0;
                let is_selected = is_active_row && self.selected_step == Some(i);
                let (response, painter) = ui.allocate_painter(Vec2::splat(l.size), Sense::click_and_drag());
                let rect = response.rect;

                // --- Drop: browser file ---
                let file_payload = egui::DragAndDrop::payload::<PathBuf>(ui.ctx());
                let step_payload = egui::DragAndDrop::payload::<DragStep>(ui.ctx());
                let pointer_pos = ui.input(|inp| inp.pointer.hover_pos());
                let pointer_in_rect = pointer_pos.map(|p| rect.contains(p)).unwrap_or(false);
                let any_drop_hover = pointer_in_rect
                    && (file_payload.is_some() || step_payload.is_some());

                let released_in_rect = pointer_in_rect && ui.input(|inp| inp.pointer.any_released());
                if let (true, Some(path)) = (released_in_rect, &file_payload) {
                    actions.push(KeyboardSequencerAction::LoadStepSample {
                        step: i,
                        layer,
                        path: (**path).clone(),
                    });
                }
                let valid_step_drop = step_payload.as_ref()
                    .filter(|src| (src.0 != i || src.1 != layer) && self.drum_steps[src.0].layers[src.1].sample_name.is_some());
                if let (true, Some(src)) = (released_in_rect, valid_step_drop) {
                    actions.push(KeyboardSequencerAction::MoveStepSample {
                        from_step: src.0, from_layer: src.1, to_step: i, to_layer: layer,
                    });
                }

                // --- Click: select step, toggle active ---
                if response.clicked() && !any_drop_hover {
                    self.selected_step = Some(i);
                    self.active_row = SequencerRow::Drum;
                    self.drum_steps[i].active = !active;
                    actions.push(KeyboardSequencerAction::ToggleDrumStep(i));
                }

                // --- Drag: initiate step drag if step has a sample ---
                if response.dragged() && has_sample {
                    egui::DragAndDrop::set_payload(ui.ctx(), DragStep(i, layer));
                }

                // --- Right-click context menu ---
                response.context_menu(|ui| {
                    if has_sample && ui.button("Copy").clicked() {
                        actions.push(KeyboardSequencerAction::CopyDrumStep { step: i, layer });
                        ui.close_menu();
                    }
                    if clipboard.content().is_some() && ui.button("Paste").clicked() {
                        actions.extend(self.paste_from_clipboard(clipboard, i, layer));
                        ui.close_menu();
                    }
                    if has_sample && ui.button("Clear").clicked() {
                        self.drum_steps[i].layers[layer].sample_name = None;
                        self.drum_steps[i].active = self.drum_steps[i].has_any_sample();
                        actions.push(KeyboardSequencerAction::ToggleDrumStep(i));
                        ui.close_menu();
                    }
                });

                // --- Visual ---
                let bg = if any_drop_hover {
                    Color32::from_rgb(80, 120, 180)
                } else if is_triggered {
                    PAD_PRESSED
                } else {
                    match (active, is_current) {
                        (true, true) => PAD_ACTIVE_STEP,
                        (true, false) => PAD_ACTIVE,
                        (false, true) => PAD_CURRENT,
                        (false, false) => PAD_BG,
                    }
                };

                // DR row always shows step number (1, 2, 3...), sample names shown in layers above
                let label = DRUM_KEY_LABELS[i].to_string();

                self.draw_pad(&painter, rect, bg, &label, active || is_triggered);

                if is_selected {
                    painter.rect_stroke(rect, l.radius, Stroke::new(2.0, Color32::from_rgb(130, 170, 220)), egui::StrokeKind::Outside);
                }

                ui.add_space(l.spacing);
            }
        });

        actions
    }

    pub(super) fn draw_expanded_grid(
        &mut self,
        ui: &mut Ui,
        is_playing: bool,
        _clipboard: &DawClipboard,
    ) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let l = self.layout();
        let step_count = self.drum_steps.len();
        let cell_size = l.size * 0.8;
        let sample_btn_w = l.label_w * 1.8;

        let active_row = self.active_row;

        // Draw layers bottom-to-top (layer 0 at bottom, layer 11 at top)
        for row in (0..12).rev() {
            let is_active_row = active_row == SequencerRow::DrumLayer(row);
            let row_has_sample = self.row_samples[row].is_some();

            ui.horizontal(|ui| {
                // --- Sample drop zone button ---
                let (sample_resp, sample_painter) = ui.allocate_painter(
                    Vec2::new(sample_btn_w, cell_size),
                    Sense::click_and_drag(),
                );
                let sample_rect = sample_resp.rect;

                // Handle file drop and row-to-row drop on sample button
                let file_payload = egui::DragAndDrop::payload::<PathBuf>(ui.ctx());
                let row_payload = egui::DragAndDrop::payload::<DragRowSample>(ui.ctx());
                let pointer_pos = ui.input(|inp| inp.pointer.hover_pos());
                let pointer_in_sample = pointer_pos.map(|p| sample_rect.contains(p)).unwrap_or(false);
                let drop_hover = pointer_in_sample && (file_payload.is_some() || row_payload.is_some());

                let released_in_sample = pointer_in_sample && ui.input(|inp| inp.pointer.any_released());
                if let (true, Some(path)) = (released_in_sample, &file_payload) {
                    actions.push(KeyboardSequencerAction::LoadRowSample {
                        row,
                        path: (**path).clone(),
                    });
                }
                let valid_row_drop = row_payload.as_ref()
                    .filter(|src| src.0 != row && self.row_samples[src.0].is_some());
                if let (true, Some(src)) = (released_in_sample, valid_row_drop) {
                    actions.push(KeyboardSequencerAction::MoveRowSample {
                        from_row: src.0,
                        to_row: row,
                    });
                }

                // Drag to move sample to another row
                if sample_resp.dragged() && row_has_sample {
                    egui::DragAndDrop::set_payload(ui.ctx(), DragRowSample(row));
                }

                let row_enabled = self.row_enabled[row];
                let ctrl_held = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
                let row_is_selected = self.selected_rows.contains(&row);

                // Click handling: Ctrl+Click = multi-select, regular click = toggle mute
                if sample_resp.clicked() {
                    self.active_drum_layer = row;
                    self.active_row = SequencerRow::DrumLayer(row);

                    if ctrl_held {
                        // Ctrl+Click: toggle selection (don't mute)
                        if row_is_selected {
                            self.selected_rows.remove(&row);
                        } else {
                            self.selected_rows.insert(row);
                        }
                    } else if row_has_sample {
                        // Regular click on sample: toggle mute for this row and all selected
                        let new_enabled = !row_enabled;
                        self.row_enabled[row] = new_enabled;
                        actions.push(KeyboardSequencerAction::ToggleRowEnabled { row });

                        // Also toggle all selected rows to match
                        for &sel_row in &self.selected_rows.clone() {
                            if sel_row != row && self.row_samples[sel_row].is_some() {
                                self.row_enabled[sel_row] = new_enabled;
                                actions.push(KeyboardSequencerAction::ToggleRowEnabled { row: sel_row });
                            }
                        }
                        self.selected_rows.clear();
                    }
                }

                // Right-click context menu
                sample_resp.context_menu(|ui| {
                    if row_has_sample {
                        let mute_label = if row_enabled { "Mute" } else { "Unmute" };
                        if ui.button(mute_label).clicked() {
                            let new_enabled = !row_enabled;
                            self.row_enabled[row] = new_enabled;
                            actions.push(KeyboardSequencerAction::ToggleRowEnabled { row });
                            // Also apply to selected rows
                            for &sel_row in &self.selected_rows.clone() {
                                if sel_row != row && self.row_samples[sel_row].is_some() {
                                    self.row_enabled[sel_row] = new_enabled;
                                    actions.push(KeyboardSequencerAction::ToggleRowEnabled { row: sel_row });
                                }
                            }
                            ui.close_menu();
                        }
                    }
                    if row_has_sample && ui.button("Copy").clicked() {
                        actions.push(KeyboardSequencerAction::CopyRowSample { row });
                        ui.close_menu();
                    }
                    if ui.button("Paste").clicked() {
                        actions.push(KeyboardSequencerAction::PasteRowSample { row });
                        ui.close_menu();
                    }
                    if row_has_sample && ui.button("Clear").clicked() {
                        actions.push(KeyboardSequencerAction::ClearRowSample { row });
                        ui.close_menu();
                    }
                });

                // Visual for sample button - enabled rows are brighter, multi-selected have highlight
                let sample_bg = if drop_hover {
                    Color32::from_rgb(80, 120, 180)
                } else if row_is_selected {
                    Color32::from_rgb(70, 90, 110)  // Multi-selected: blue tint
                } else if row_has_sample && row_enabled {
                    Color32::from_rgb(60, 70, 50)  // Enabled: greenish tint
                } else if is_active_row {
                    Color32::from_rgb(50, 50, 55)
                } else {
                    Color32::from_rgb(35, 35, 40)
                };
                sample_painter.rect_filled(sample_rect, l.radius * 0.5, sample_bg);
                sample_painter.rect_stroke(sample_rect, l.radius * 0.5, Stroke::new(0.5, PAD_BORDER), egui::StrokeKind::Outside);

                // Selection highlight (active row or multi-selected)
                if is_active_row || row_is_selected {
                    let highlight_color = if row_is_selected {
                        Color32::from_rgb(100, 140, 180)  // Multi-select: lighter blue
                    } else {
                        Color32::from_rgb(130, 170, 220)  // Active: standard blue
                    };
                    sample_painter.rect_stroke(sample_rect, l.radius * 0.5, Stroke::new(1.5, highlight_color), egui::StrokeKind::Outside);
                }

                let sample_label = self.row_samples[row].as_ref()
                    .map(|n| truncate_label(n, 6))
                    .unwrap_or_else(|| "---".to_string());
                // Muted rows show dimmed text
                let sample_text_color = match (row_has_sample, row_enabled) {
                    (true, true) => LABEL_BRIGHT,
                    (true, false) => Color32::from_gray(80),  // Muted: dim
                    _ => LABEL_DIM,
                };
                sample_painter.text(
                    sample_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    sample_label,
                    egui::FontId::proportional(l.font_pad * 0.7),
                    sample_text_color,
                );

                ui.add_space(2.0);

                // --- Row number label ---
                ui.allocate_ui(Vec2::new(l.label_w * 0.6, cell_size), |ui| {
                    ui.centered_and_justified(|ui| {
                        let label_color = if is_active_row { LABEL_BRIGHT } else { LABEL_DIM };
                        ui.colored_label(label_color, format!("{}", row + 1));
                    });
                });

                // --- Step cells (just timing toggles, no individual samples) ---
                for step in 0..step_count {
                    let step_active = self.drum_steps[step].layers[row].active;
                    let is_current = is_playing && self.current_step == step;
                    let is_triggered = is_active_row && (self.triggered_steps & (1 << step)) != 0;
                    let cell_key = (row, step);
                    let cell_is_multi_selected = self.selected_cells.contains(&cell_key);

                    let (response, painter) = ui.allocate_painter(
                        Vec2::new(l.size, cell_size),
                        Sense::click(),
                    );
                    let rect = response.rect;

                    let ctrl_held = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);

                    // Click: Ctrl+Click = multi-select, regular click = toggle active
                    if response.clicked() {
                        self.selected_step = Some(step);
                        self.active_drum_layer = row;
                        self.active_row = SequencerRow::DrumLayer(row);

                        if ctrl_held {
                            // Ctrl+Click: toggle cell selection (don't toggle active)
                            if cell_is_multi_selected {
                                self.selected_cells.remove(&cell_key);
                            } else {
                                self.selected_cells.insert(cell_key);
                            }
                        } else {
                            // Regular click: toggle this cell and all selected cells
                            let new_active = !step_active;
                            self.drum_steps[step].layers[row].active = new_active;
                            actions.push(KeyboardSequencerAction::ToggleDrumStep(step));

                            // Also toggle all multi-selected cells to match
                            for &(sel_row, sel_step) in &self.selected_cells.clone() {
                                if (sel_row, sel_step) != cell_key {
                                    self.drum_steps[sel_step].layers[sel_row].active = new_active;
                                    actions.push(KeyboardSequencerAction::ToggleDrumStep(sel_step));
                                }
                            }
                            self.selected_cells.clear();
                        }
                    }

                    // Visual
                    let bg = if cell_is_multi_selected {
                        Color32::from_rgb(70, 90, 110)  // Multi-selected: blue tint
                    } else if is_triggered {
                        PAD_PRESSED
                    } else {
                        match (step_active, is_current) {
                            (true, true) => PAD_ACTIVE_STEP,
                            (true, false) => PAD_ACTIVE,
                            (false, true) => PAD_CURRENT,
                            (false, false) => PAD_BG,
                        }
                    };

                    painter.rect_filled(rect, l.radius * 0.5, bg);
                    painter.rect_stroke(rect, l.radius * 0.5, Stroke::new(0.5, PAD_BORDER), egui::StrokeKind::Outside);

                    // Selection highlight (single selection or multi-selected)
                    let is_selected = is_active_row && self.selected_step == Some(step);
                    if is_selected || cell_is_multi_selected {
                        let highlight_color = if cell_is_multi_selected {
                            Color32::from_rgb(100, 140, 180)
                        } else {
                            Color32::from_rgb(130, 170, 220)
                        };
                        painter.rect_stroke(rect, l.radius * 0.5, Stroke::new(2.0, highlight_color), egui::StrokeKind::Outside);
                    }

                    // Show sample name on all steps where row has sample
                    // Active steps = bright, inactive = dimmed, triggered = dark on yellow
                    if let Some(name) = &self.row_samples[row] {
                        let text_color = if is_triggered {
                            Color32::from_rgb(40, 35, 20)
                        } else if step_active {
                            LABEL_BRIGHT
                        } else {
                            Color32::from_gray(60)
                        };
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            truncate_label(name, 3),
                            egui::FontId::proportional(l.font_pad * 0.7),
                            text_color,
                        );
                    }

                    ui.add_space(l.spacing);
                }
            });
            ui.add_space(1.0);
        }

        actions
    }

    pub(super) fn draw_melodic_row(&mut self, ui: &mut Ui, label: &str, keys: &[Key], _base_pitch: u8, is_playing: bool, is_active_row: bool, row: SequencerRow) {
        let l = self.layout();
        let sample_btn_w = l.label_w * 1.8;
        ui.horizontal(|ui| {
            // Empty spacer matching sample button width
            ui.allocate_space(Vec2::new(sample_btn_w, l.size));
            ui.add_space(2.0);
            // Label matching row number width (use painter for consistent font with DR)
            let (_, label_painter) = ui.allocate_painter(Vec2::new(l.label_w * 0.6, l.size), Sense::hover());
            let label_color = if is_active_row { LABEL_BRIGHT } else { LABEL_DIM };
            label_painter.text(
                label_painter.clip_rect().center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(l.font_pad),
                label_color,
            );

            for (i, &key) in keys.iter().enumerate() {
                let is_pressed = self.pressed_keys.contains_key(&key);
                let is_black = self.is_step_accidental(i);
                let is_current = is_playing && self.current_step == i;
                let is_selected = is_active_row && self.selected_step == Some(i);

                let (response, painter) = ui.allocate_painter(Vec2::splat(l.size), Sense::click());
                let rect = response.rect;

                // Click to select this cell
                if response.clicked() {
                    self.selected_step = Some(i);
                    self.active_row = row;
                }

                let bg = if is_pressed {
                    PAD_PRESSED
                } else if is_current {
                    PAD_CURRENT
                } else if is_black {
                    PAD_BLACK
                } else {
                    PAD_BG
                };

                let text_color = if is_pressed || is_current {
                    LABEL_BRIGHT
                } else if is_black {
                    Color32::from_gray(90)
                } else {
                    LABEL_DIM
                };

                painter.rect_filled(rect, l.radius, bg);
                painter.rect_stroke(rect, l.radius, Stroke::new(1.0, PAD_BORDER), egui::StrokeKind::Outside);

                // Selection highlight
                if is_selected {
                    painter.rect_stroke(rect, l.radius, Stroke::new(2.0, Color32::from_rgb(130, 170, 220)), egui::StrokeKind::Outside);
                }

                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    self.note_name_for_step(i),
                    egui::FontId::proportional(l.font_pad),
                    text_color,
                );

                ui.add_space(l.spacing);
            }
        });
    }

    /// Draw a single Factory Rat-style pad
    pub(super) fn draw_pad(&self, painter: &egui::Painter, rect: Rect, bg: Color32, label: &str, lit: bool) {
        let l = self.layout();
        painter.rect_filled(rect, l.radius, bg);
        painter.rect_stroke(rect, l.radius, Stroke::new(1.0, PAD_BORDER), egui::StrokeKind::Outside);

        if lit {
            let glow = rect.shrink(l.glow_inset);
            painter.rect_filled(glow, l.radius - 2.0, Color32::from_rgba_premultiplied(255, 240, 160, 40));
        }

        let text_color = if lit { Color32::from_rgb(40, 35, 20) } else { LABEL_DIM };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(l.font_pad),
            text_color,
        );
    }
}
