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
    ) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let l = self.layout();
        let layer = self.active_drum_layer;

        ui.horizontal(|ui| {
            // DR label (same width as C3/C4/C5 labels for alignment)
            ui.allocate_ui(Vec2::new(l.label_w, l.size), |ui| {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(LABEL_DIM, "DR");
                });
            });

            for i in 0..self.drum_steps.len() {
                let active = self.drum_steps[i].active;
                let has_sample = self.drum_steps[i].layers[layer].sample_name.is_some();
                let is_current = is_playing && self.current_step == i;
                let is_selected = self.selected_step == Some(i);
                let (response, painter) = ui.allocate_painter(Vec2::splat(l.size), Sense::click_and_drag());
                let rect = response.rect;

                // --- Drop: browser file ---
                let file_payload = egui::DragAndDrop::payload::<PathBuf>(ui.ctx());
                let step_payload = egui::DragAndDrop::payload::<DragStep>(ui.ctx());
                let pointer_pos = ui.input(|inp| inp.pointer.hover_pos());
                let pointer_in_rect = pointer_pos.map(|p| rect.contains(p)).unwrap_or(false);
                let any_drop_hover = pointer_in_rect
                    && (file_payload.is_some() || step_payload.is_some());

                if pointer_in_rect && ui.input(|inp| inp.pointer.any_released()) {
                    if let Some(path) = &file_payload {
                        actions.push(KeyboardSequencerAction::LoadStepSample {
                            step: i,
                            layer,
                            path: (**path).clone(),
                        });
                    }
                    if let Some(src) = &step_payload {
                        if (src.0 != i || src.1 != layer)
                            && self.drum_steps[src.0].layers[src.1].sample_name.is_some()
                        {
                            actions.push(KeyboardSequencerAction::MoveStepSample {
                                from_step: src.0, from_layer: src.1, to_step: i, to_layer: layer,
                            });
                        }
                    }
                }

                // --- Click: select step, toggle active ---
                if response.clicked() && !any_drop_hover {
                    self.selected_step = Some(i);
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
                        if !self.drum_steps[i].has_any_sample() {
                            self.drum_steps[i].active = false;
                        }
                        actions.push(KeyboardSequencerAction::ToggleDrumStep(i));
                        ui.close_menu();
                    }
                });

                // --- Visual ---
                let bg = if any_drop_hover {
                    Color32::from_rgb(80, 120, 180)
                } else {
                    match (active, is_current) {
                        (true, true) => PAD_ACTIVE_STEP,
                        (true, false) => PAD_ACTIVE,
                        (false, true) => PAD_CURRENT,
                        (false, false) => PAD_BG,
                    }
                };

                let label = self.drum_steps[i].layers[layer].sample_name.as_ref()
                    .map(|n| truncate_label(n, 4))
                    .unwrap_or_else(|| DRUM_KEY_LABELS[i].to_string());

                self.draw_pad(&painter, rect, bg, &label, active);

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
        clipboard: &DawClipboard,
    ) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let l = self.layout();
        let step_count = self.drum_steps.len();
        let cell_size = l.size * 0.8;

        // Draw layers bottom-to-top (layer 0 at bottom, layer 11 at top)
        for row in (0..12).rev() {
            ui.horizontal(|ui| {
                ui.allocate_ui(Vec2::new(l.label_w, cell_size), |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(LABEL_DIM, format!("{}", row + 1));
                    });
                });

                for step in 0..step_count {
                    let layer_ref = &self.drum_steps[step].layers[row];
                    let has_sample = layer_ref.sample_name.is_some();
                    let layer_active = layer_ref.active;
                    let is_active_layer = self.active_drum_layer == row;
                    let is_selected = self.selected_step == Some(step) && is_active_layer;
                    let is_current = is_playing && self.current_step == step;

                    let (response, painter) = ui.allocate_painter(
                        Vec2::new(l.size, cell_size),
                        Sense::click_and_drag(),
                    );
                    let rect = response.rect;

                    // --- Drop ---
                    let file_payload = egui::DragAndDrop::payload::<PathBuf>(ui.ctx());
                    let step_payload = egui::DragAndDrop::payload::<DragStep>(ui.ctx());
                    let pointer_pos = ui.input(|inp| inp.pointer.hover_pos());
                    let pointer_in_rect = pointer_pos.map(|p| rect.contains(p)).unwrap_or(false);
                    let any_drop_hover = pointer_in_rect
                        && (file_payload.is_some() || step_payload.is_some());

                    if pointer_in_rect && ui.input(|inp| inp.pointer.any_released()) {
                        if let Some(path) = &file_payload {
                            actions.push(KeyboardSequencerAction::LoadStepSample {
                                step, layer: row, path: (**path).clone(),
                            });
                        }
                        if let Some(src) = &step_payload {
                            if (src.0 != step || src.1 != row)
                                && self.drum_steps[src.0].layers[src.1].sample_name.is_some()
                            {
                                actions.push(KeyboardSequencerAction::MoveStepSample {
                                    from_step: src.0, from_layer: src.1, to_step: step, to_layer: row,
                                });
                            }
                        }
                    }

                    // --- Click: toggle layer active + select ---
                    if response.clicked() && !any_drop_hover {
                        self.drum_steps[step].layers[row].active = !self.drum_steps[step].layers[row].active;
                        self.selected_step = Some(step);
                        self.active_drum_layer = row;
                        actions.push(KeyboardSequencerAction::ToggleDrumStep(step));
                    }

                    // --- Drag ---
                    if response.dragged() && has_sample {
                        egui::DragAndDrop::set_payload(ui.ctx(), DragStep(step, row));
                    }

                    // --- Right-click ---
                    response.context_menu(|ui| {
                        if has_sample && ui.button("Copy").clicked() {
                            actions.push(KeyboardSequencerAction::CopyDrumStep { step, layer: row });
                            ui.close_menu();
                        }
                        if clipboard.content().is_some() && ui.button("Paste").clicked() {
                            actions.extend(self.paste_from_clipboard(clipboard, step, row));
                            ui.close_menu();
                        }
                        if has_sample && ui.button("Clear").clicked() {
                            self.drum_steps[step].layers[row].sample_name = None;
                            self.drum_steps[step].layers[row].active = false;
                            if !self.drum_steps[step].has_any_sample() {
                                self.drum_steps[step].active = false;
                            }
                            actions.push(KeyboardSequencerAction::ClearStepSample { step, layer: row });
                            ui.close_menu();
                        }
                    });

                    // --- Visual ---
                    let bg = if any_drop_hover {
                        Color32::from_rgb(80, 120, 180)
                    } else {
                        match (layer_active, is_current) {
                            (true, true) => PAD_ACTIVE_STEP,
                            (true, false) => PAD_ACTIVE,
                            (false, true) => PAD_CURRENT,
                            (false, false) => PAD_BG,
                        }
                    };

                    painter.rect_filled(rect, l.radius * 0.5, bg);
                    painter.rect_stroke(rect, l.radius * 0.5, Stroke::new(0.5, PAD_BORDER), egui::StrokeKind::Outside);

                    let label_text = self.drum_steps[step].layers[row].sample_name.as_ref()
                        .filter(|_| has_sample)
                        .map(|name| truncate_label(name, 3))
                        .unwrap_or_default();
                    if !label_text.is_empty() {
                        let text_color = if layer_active { Color32::from_rgb(40, 35, 20) } else { LABEL_DIM };
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            label_text,
                            egui::FontId::proportional(l.font_pad * 0.8),
                            text_color,
                        );
                    }

                    if is_selected {
                        painter.rect_stroke(rect, l.radius * 0.5, Stroke::new(1.5, Color32::from_rgb(130, 170, 220)), egui::StrokeKind::Outside);
                    }

                    ui.add_space(l.spacing);
                }
            });
            ui.add_space(1.0);
        }

        actions
    }

    pub(super) fn draw_melodic_row(&self, ui: &mut Ui, label: &str, keys: &[Key], _base_pitch: u8, is_playing: bool) {
        let l = self.layout();
        ui.horizontal(|ui| {
            ui.allocate_ui(Vec2::new(l.label_w, l.size), |ui| {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(LABEL_DIM, label);
                });
            });

            for (i, &key) in keys.iter().enumerate() {
                let is_pressed = self.pressed_keys.contains_key(&key);
                let is_black = self.is_step_accidental(i);
                let is_current = is_playing && self.current_step == i;

                let (response, painter) = ui.allocate_painter(Vec2::splat(l.size), Sense::click());
                let rect = response.rect;

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
