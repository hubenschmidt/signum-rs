use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};
use hallucinator_core::MidiClip;

use super::PianoRollPanel;

impl PianoRollPanel {
    pub(super) fn draw_piano_keys(&self, painter: &egui::Painter, rect: Rect) {
        let pitch_max = (self.visible_pitch_min + self.visible_pitches).min(127);

        for pitch in self.visible_pitch_min..pitch_max {
            let y = self.pitch_to_y(pitch, rect);
            let key_rect = Rect::from_min_size(
                Pos2::new(rect.left(), y),
                Vec2::new(rect.width(), self.key_height),
            );

            let is_black = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
            let is_active = self.active_pitches.contains(&pitch);

            let color = if is_active {
                Color32::from_rgb(255, 140, 0)
            } else if is_black {
                Color32::from_gray(30)
            } else {
                Color32::from_gray(60)
            };

            painter.rect_filled(key_rect, 0.0, color);
            painter.rect_stroke(key_rect, 0.0, Stroke::new(0.5, Color32::from_gray(20)), StrokeKind::Inside);

            // Label C notes
            if pitch % 12 == 0 {
                let octave = (pitch as i32 / 12) - 1;
                let text_color = if is_active { Color32::BLACK } else { Color32::WHITE };
                painter.text(
                    Pos2::new(rect.left() + 2.0, y + 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("C{}", octave),
                    egui::FontId::proportional(9.0),
                    text_color,
                );
            }
        }
    }

    pub(super) fn draw_grid(&self, painter: &egui::Painter, rect: Rect, beats_visible: f64) {
        let start_beat = self.scroll_x;
        let end_beat = start_beat + beats_visible;

        let grid_step = self.grid_subdivision;
        let pixels_per_grid = self.pixels_per_beat as f64 * grid_step;

        // Only draw if grid lines have minimum spacing
        if pixels_per_grid >= 8.0 {
            let mut pos = (start_beat / grid_step).floor() * grid_step;
            while pos <= end_beat {
                let x = rect.left() + ((pos - start_beat) * self.pixels_per_beat as f64) as f32;

                let is_bar = (pos.round() as i32) % 4 == 0 && (pos - pos.round()).abs() < 0.001;
                let is_beat = (pos - pos.round()).abs() < 0.001;

                let stroke = if is_bar {
                    Stroke::new(1.0, Color32::from_gray(90))
                } else if is_beat {
                    Stroke::new(0.5, Color32::from_gray(65))
                } else {
                    Stroke::new(0.5, Color32::from_gray(45))
                };

                painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], stroke);
                pos += grid_step;
            }
        }

        // Draw horizontal lines (pitches)
        let pitch_max = (self.visible_pitch_min + self.visible_pitches).min(127);
        for pitch in self.visible_pitch_min..pitch_max {
            let y = self.pitch_to_y(pitch, rect) + self.key_height;
            let is_c = pitch % 12 == 0;
            let stroke = if is_c {
                Stroke::new(1.0, Color32::from_gray(60))
            } else {
                Stroke::new(0.5, Color32::from_gray(40))
            };
            painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], stroke);
        }
    }

    pub(super) fn draw_notes(&self, painter: &egui::Painter, rect: Rect, clip: &MidiClip) {
        for (idx, note) in clip.notes.iter().enumerate() {
            let start_beat = note.start_tick as f64 / clip.ppq as f64;
            let duration_beats = note.duration_ticks as f64 / clip.ppq as f64;

            if start_beat + duration_beats < self.scroll_x {
                continue;
            }
            if note.pitch < self.visible_pitch_min || note.pitch >= self.visible_pitch_min + self.visible_pitches {
                continue;
            }

            let x = rect.left() + ((start_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;
            let y = self.pitch_to_y(note.pitch, rect);
            let width = (duration_beats * self.pixels_per_beat as f64) as f32;

            let note_rect = Rect::from_min_size(
                Pos2::new(x, y),
                Vec2::new(width.max(4.0), self.key_height - 1.0),
            );

            let visible_rect = note_rect.intersect(rect);
            if visible_rect.width() <= 0.0 {
                continue;
            }

            let is_selected = self.selected_notes.contains(&idx);
            let color = if is_selected {
                Color32::from_rgb(100, 200, 255)
            } else {
                Color32::from_rgb(80, 160, 220)
            };

            painter.rect_filled(visible_rect, 2.0, color);
            painter.rect_stroke(visible_rect, 2.0, Stroke::new(1.0, Color32::from_rgb(40, 80, 120)), StrokeKind::Inside);
        }
    }

    pub(super) fn draw_playhead(
        &self,
        painter: &egui::Painter,
        grid_rect: Rect,
        clip_start_sample: u64,
        clip_length_samples: u64,
        playback_position: u64,
        samples_per_beat: f64,
    ) {
        let clip_end_sample = clip_start_sample + clip_length_samples;
        if playback_position < clip_start_sample || playback_position > clip_end_sample {
            return;
        }

        let position_in_clip = playback_position - clip_start_sample;
        let position_beat = position_in_clip as f64 / samples_per_beat;
        let playhead_x = grid_rect.left() + ((position_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;

        if playhead_x < grid_rect.left() || playhead_x > grid_rect.right() {
            return;
        }

        let playhead_color = Color32::from_rgb(255, 100, 100);

        painter.line_segment(
            [Pos2::new(playhead_x, grid_rect.top()), Pos2::new(playhead_x, grid_rect.bottom())],
            Stroke::new(2.0, playhead_color),
        );

        let triangle = vec![
            Pos2::new(playhead_x, grid_rect.top()),
            Pos2::new(playhead_x - 6.0, grid_rect.top() - 8.0),
            Pos2::new(playhead_x + 6.0, grid_rect.top() - 8.0),
        ];
        painter.add(egui::Shape::convex_polygon(triangle, playhead_color, Stroke::NONE));
    }

    pub(super) fn draw_loop_selection(&self, painter: &egui::Painter, grid_rect: Rect) {
        let Some(ref selection) = self.loop_selection else { return };

        let start_x = grid_rect.left() + ((selection.start_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;
        let end_x = grid_rect.left() + ((selection.end_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;

        if end_x <= grid_rect.left() || start_x >= grid_rect.right() {
            return;
        }

        // Selection highlight
        let sel_rect = Rect::from_min_max(
            Pos2::new(start_x.max(grid_rect.left()), grid_rect.top()),
            Pos2::new(end_x.min(grid_rect.right()), grid_rect.bottom()),
        );
        painter.rect_filled(sel_rect, 0.0, Color32::from_rgba_unmultiplied(100, 150, 255, 30));
        painter.rect_stroke(sel_rect, 0.0, Stroke::new(2.0, Color32::from_rgb(100, 150, 255)), StrokeKind::Inside);

        let handle_color = Color32::from_rgb(80, 130, 220);

        // Start handle
        if start_x >= grid_rect.left() {
            let triangle = vec![
                Pos2::new(start_x, grid_rect.top()),
                Pos2::new(start_x - 8.0, grid_rect.top() - 12.0),
                Pos2::new(start_x + 8.0, grid_rect.top() - 12.0),
            ];
            painter.add(egui::Shape::convex_polygon(triangle, handle_color, Stroke::NONE));
            painter.line_segment(
                [Pos2::new(start_x, grid_rect.top()), Pos2::new(start_x, grid_rect.bottom())],
                Stroke::new(2.0, handle_color),
            );
        }

        // End handle
        if end_x <= grid_rect.right() {
            let triangle = vec![
                Pos2::new(end_x, grid_rect.top()),
                Pos2::new(end_x - 8.0, grid_rect.top() - 12.0),
                Pos2::new(end_x + 8.0, grid_rect.top() - 12.0),
            ];
            painter.add(egui::Shape::convex_polygon(triangle, handle_color, Stroke::NONE));
            painter.line_segment(
                [Pos2::new(end_x, grid_rect.top()), Pos2::new(end_x, grid_rect.bottom())],
                Stroke::new(2.0, handle_color),
            );
        }
    }
}
