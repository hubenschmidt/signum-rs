//! Pattern bank panel - 4x4 grid of patterns per track (Hapax-style)

use egui::{Color32, Key, Rect, Sense, Stroke, Ui, Vec2};
use signum_core::PatternSlot;

/// QWERTY keyboard mapping to pattern slots (0-indexed)
const PATTERN_KEYS: [(Key, usize); 16] = [
    (Key::Q, 0), (Key::W, 1), (Key::E, 2), (Key::R, 3),
    (Key::A, 4), (Key::S, 5), (Key::D, 6), (Key::F, 7),
    (Key::Z, 8), (Key::X, 9), (Key::C, 10), (Key::V, 11),
    (Key::Num1, 12), (Key::Num2, 13), (Key::Num3, 14), (Key::Num4, 15),
];

/// Action returned from pattern bank
#[derive(Clone)]
pub enum PatternBankAction {
    None,
    SelectPattern(usize),
    QueuePattern(usize),
    EditPattern(usize),
    CopyPattern { from: usize, to: usize },
    ClearPattern(usize),
}

/// Pattern bank panel state
pub struct PatternBankPanel {
    selected_pattern: usize,
    playing_pattern: usize,
    queued_pattern: Option<usize>,
    drag_source: Option<usize>,
}

impl PatternBankPanel {
    pub fn new() -> Self {
        Self {
            selected_pattern: 0,
            playing_pattern: 0,
            queued_pattern: None,
            drag_source: None,
        }
    }

    pub fn set_playing_pattern(&mut self, idx: usize) {
        self.playing_pattern = idx;
    }

    pub fn set_queued_pattern(&mut self, queued: Option<usize>) {
        self.queued_pattern = queued;
    }

    pub fn selected_pattern(&self) -> usize {
        self.selected_pattern
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        track_name: Option<&str>,
        patterns: Option<&[PatternSlot; 16]>,
    ) -> PatternBankAction {
        let mut action = PatternBankAction::None;

        ui.horizontal(|ui| {
            ui.heading("Patterns");
            if let Some(name) = track_name {
                ui.separator();
                ui.label(name);
            }
        });

        ui.separator();

        let Some(patterns) = patterns else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a MIDI track to view patterns");
            });
            return action;
        };

        // QWERTY keyboard pattern selection
        for &(key, pattern_idx) in &PATTERN_KEYS {
            if ui.input(|i| i.key_pressed(key)) {
                let queue = ui.input(|i| i.modifiers.alt);
                if queue {
                    action = PatternBankAction::QueuePattern(pattern_idx);
                } else {
                    self.selected_pattern = pattern_idx;
                    action = PatternBankAction::SelectPattern(pattern_idx);
                }
            }
        }

        let cell_size = 36.0;
        let spacing = 3.0;

        // 4x4 grid - must be in vertical block to work inside horizontal parent
        ui.vertical(|ui| {
            for row in 0..4 {
                ui.horizontal(|ui| {
                    for col in 0..4 {
                        let idx = row * 4 + col;
                        let cell_action = self.draw_pattern_cell(ui, idx, &patterns[idx], cell_size);
                        if !matches!(cell_action, PatternBankAction::None) {
                            action = cell_action;
                        }
                        if col < 3 {
                            ui.add_space(spacing);
                        }
                    }
                });
                if row < 3 {
                    ui.add_space(spacing);
                }
            }
            ui.add_space(2.0);
            ui.small("QWER/ASDF/ZXCV/1234  Alt=Queue");
        });

        action
    }

    fn draw_pattern_cell(
        &mut self,
        ui: &mut Ui,
        idx: usize,
        pattern: &PatternSlot,
        size: f32,
    ) -> PatternBankAction {
        let mut action = PatternBankAction::None;

        let (response, painter) = ui.allocate_painter(Vec2::splat(size), Sense::click_and_drag());
        let rect = response.rect;

        let is_selected = self.selected_pattern == idx;
        let is_playing = self.playing_pattern == idx;
        let is_queued = self.queued_pattern == Some(idx);
        let is_empty = pattern.is_empty();

        // Background color indicates state
        let bg_color = match (is_playing, is_queued, is_empty) {
            (true, _, _) => Color32::from_rgb(60, 120, 80),   // Playing - green
            (_, true, _) => Color32::from_rgb(120, 100, 60),  // Queued - amber
            (_, _, true) => Color32::from_gray(35),           // Empty
            _ => Color32::from_rgb(50, 60, 80),               // Has content
        };

        painter.rect_filled(rect, 3.0, bg_color);

        // Border for selection
        let border_color = if is_selected {
            Color32::from_rgb(150, 180, 220)
        } else {
            Color32::from_gray(60)
        };
        let border_width = if is_selected { 2.0 } else { 1.0 };
        painter.rect_stroke(rect, 3.0, Stroke::new(border_width, border_color), egui::StrokeKind::Outside);

        // Just show pattern number, color indicates status
        let text_color = if is_empty { Color32::from_gray(80) } else { Color32::WHITE };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{}", idx + 1),
            egui::FontId::proportional(12.0),
            text_color,
        );

        // Handle interactions
        if response.clicked() {
            if ui.input(|i| i.modifiers.shift) {
                action = PatternBankAction::QueuePattern(idx);
            } else {
                self.selected_pattern = idx;
                action = PatternBankAction::SelectPattern(idx);
            }
        }

        if response.double_clicked() {
            action = PatternBankAction::EditPattern(idx);
        }

        if response.secondary_clicked() {
            action = PatternBankAction::ClearPattern(idx);
        }

        // Drag and drop for copying
        if response.drag_started() {
            self.drag_source = Some(idx);
        }

        if response.hovered() && ui.input(|i| i.pointer.any_released()) {
            if let Some(from) = self.drag_source.take() {
                if from != idx {
                    action = PatternBankAction::CopyPattern { from, to: idx };
                }
            }
        }

        action
    }
}

impl Default for PatternBankPanel {
    fn default() -> Self {
        Self::new()
    }
}
