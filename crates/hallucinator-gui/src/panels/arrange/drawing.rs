use egui::{Color32, Rect, Sense, Stroke, Ui, Vec2};
use hallucinator_core::ClipId;

use super::types::{ArrangeAction, ArrangeContext};
use super::ArrangePanel;
use crate::panels::timeline::RecordingPreview;

impl ArrangePanel {
    pub(super) fn draw_track_backgrounds(
        &self,
        painter: &egui::Painter,
        ctx: &ArrangeContext,
        tracks: &[hallucinator_core::Track],
        selected_track_idx: Option<usize>,
    ) {
        for (track_idx, _track) in tracks.iter().enumerate() {
            let track_y = ctx.track_area_top + (track_idx as f32 * self.track_height) - self.vertical_scroll;

            if track_y + self.track_height < ctx.track_area_top || track_y > ctx.rect.bottom() {
                continue;
            }

            let track_rect = Rect::from_min_size(
                egui::pos2(ctx.rect.left(), track_y),
                Vec2::new(ctx.rect.width(), self.track_height),
            );

            let is_selected = selected_track_idx == Some(track_idx);
            let bg_color = match (is_selected, track_idx % 2 == 0) {
                (true, true) => Color32::from_gray(28),
                (true, false) => Color32::from_gray(32),
                (false, true) => Color32::from_gray(42),
                (false, false) => Color32::from_gray(48),
            };
            painter.rect_filled(track_rect, 0.0, bg_color);

            painter.line_segment(
                [
                    egui::pos2(ctx.rect.left(), track_y + self.track_height),
                    egui::pos2(ctx.rect.right(), track_y + self.track_height),
                ],
                Stroke::new(1.0, Color32::from_gray(25)),
            );
        }

        // Fill remaining area below tracks
        let tracks_bottom = ctx.track_area_top + (tracks.len() as f32 * self.track_height);
        if tracks_bottom < ctx.rect.bottom() {
            let empty_rect = Rect::from_min_max(
                egui::pos2(ctx.rect.left(), tracks_bottom),
                ctx.rect.max,
            );
            painter.rect_filled(empty_rect, 0.0, Color32::from_gray(50));
        }
    }

    pub(super) fn draw_grid(&self, painter: &egui::Painter, ctx: &ArrangeContext) {
        if ctx.pixels_per_grid < 8.0 {
            return;
        }

        let mut pos = (ctx.start_beat / ctx.grid_step).floor() * ctx.grid_step;
        while pos <= ctx.end_beat {
            let x = ctx.rect.left() + ((pos - ctx.start_beat) * self.pixels_per_beat);

            let is_bar = (pos.round() as u32) % (ctx.time_sig_num as u32) == 0
                && (pos - pos.round()).abs() < 0.001;
            let is_beat = (pos - pos.round()).abs() < 0.001;

            let stroke = if is_bar {
                Stroke::new(1.0, Color32::from_gray(80))
            } else if is_beat {
                Stroke::new(0.5, Color32::from_gray(60))
            } else {
                Stroke::new(0.5, Color32::from_gray(45))
            };

            painter.line_segment(
                [egui::pos2(x, ctx.track_area_top), egui::pos2(x, ctx.rect.bottom())],
                stroke,
            );

            pos += ctx.grid_step;
        }
    }

    pub(super) fn draw_ruler(&self, painter: &egui::Painter, ctx: &ArrangeContext) {
        painter.rect_filled(ctx.ruler_rect, 0.0, Color32::from_gray(45));

        if ctx.pixels_per_grid >= 8.0 {
            let mut pos = (ctx.start_beat / ctx.grid_step).floor() * ctx.grid_step;
            while pos <= ctx.end_beat {
                let x = ctx.rect.left() + ((pos - ctx.start_beat) * self.pixels_per_beat);

                let is_bar = (pos.round() as u32) % (ctx.time_sig_num as u32) == 0
                    && (pos - pos.round()).abs() < 0.001;
                let is_beat = (pos - pos.round()).abs() < 0.001;

                let tick_height = if is_bar { 10.0 } else if is_beat { 6.0 } else { 3.0 };
                let tick_color = if is_bar || is_beat {
                    Color32::from_gray(120)
                } else {
                    Color32::from_gray(80)
                };

                painter.line_segment(
                    [
                        egui::pos2(x, ctx.ruler_rect.bottom() - tick_height),
                        egui::pos2(x, ctx.ruler_rect.bottom()),
                    ],
                    Stroke::new(1.0, tick_color),
                );

                if is_bar {
                    let bar = (pos.round() as u32) / (ctx.time_sig_num as u32) + 1;
                    painter.text(
                        egui::pos2(x + 4.0, ctx.ruler_rect.top() + 4.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", bar),
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }

                pos += ctx.grid_step;
            }
        }

        // Ruler bottom border
        painter.line_segment(
            [
                egui::pos2(ctx.rect.left(), ctx.ruler_rect.bottom()),
                egui::pos2(ctx.rect.right(), ctx.ruler_rect.bottom()),
            ],
            Stroke::new(1.0, Color32::from_gray(60)),
        );
    }

    pub(super) fn draw_clips(
        &self,
        painter: &egui::Painter,
        ui: &mut Ui,
        ctx: &ArrangeContext,
        tracks: &[hallucinator_core::Track],
        selected_clip: Option<(usize, ClipId)>,
        recording_preview: &Option<RecordingPreview>,
    ) -> ArrangeAction {
        let mut action = ArrangeAction::None;

        for (track_idx, track) in tracks.iter().enumerate() {
            let track_y = ctx.track_area_top + (track_idx as f32 * self.track_height) - self.vertical_scroll;

            if track_y + self.track_height < ctx.track_area_top || track_y > ctx.rect.bottom() {
                continue;
            }

            for clip in &track.clips {
                let clip_action = self.draw_audio_clip(painter, ui, clip, track_idx, track_y, ctx, selected_clip);
                if !matches!(clip_action, ArrangeAction::None) {
                    action = clip_action;
                }
            }

            for clip in &track.midi_clips {
                let clip_action = self.draw_midi_clip(painter, ui, clip, track_idx, track_y, ctx, selected_clip);
                if !matches!(clip_action, ArrangeAction::None) {
                    action = clip_action;
                }
            }

            if track.armed {
                if let Some(preview) = recording_preview {
                    self.draw_recording_preview(painter, preview, track_y, ctx);
                }
            }
        }

        action
    }

    /// Shared clip rendering: background, border, name, click handling.
    /// Returns (clip_rect if visible, action from click).
    fn draw_clip_base(
        &self,
        painter: &egui::Painter,
        ui: &mut Ui,
        clip_id: ClipId,
        clip_name: &str,
        clip_start_sample: u64,
        clip_end_sample: u64,
        track_idx: usize,
        track_y: f32,
        ctx: &ArrangeContext,
        selected_clip: Option<(usize, ClipId)>,
        fill: Color32,
        fill_selected: Color32,
        border: Color32,
    ) -> (Option<Rect>, ArrangeAction) {
        let clip_start_beat = clip_start_sample as f64 / ctx.samples_per_beat;
        let clip_end_beat = clip_end_sample as f64 / ctx.samples_per_beat;

        if clip_end_beat < ctx.start_beat as f64 || clip_start_beat > ctx.end_beat as f64 {
            return (None, ArrangeAction::None);
        }

        let clip_x = ctx.rect.left() + ((clip_start_beat as f32 - ctx.start_beat) * self.pixels_per_beat);
        let clip_width = (clip_end_beat - clip_start_beat) as f32 * self.pixels_per_beat;

        let clip_rect = Rect::from_min_size(
            egui::pos2(clip_x, track_y + 4.0),
            Vec2::new(clip_width, self.track_height - 8.0),
        );

        let is_selected = selected_clip == Some((track_idx, clip_id));
        let color = if is_selected { fill_selected } else { fill };
        let border_width = if is_selected { 2.0 } else { 1.0 };

        painter.rect_filled(clip_rect, 4.0, color);
        painter.rect_stroke(clip_rect, 4.0, Stroke::new(border_width, border), egui::StrokeKind::Outside);

        painter.text(
            egui::pos2(clip_rect.left() + 4.0, clip_rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            clip_name,
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );

        let mut action = ArrangeAction::None;
        let clip_response = ui.allocate_rect(clip_rect, Sense::click());
        if clip_response.double_clicked() {
            action = ArrangeAction::OpenClipEditor { track_idx, clip_id };
        } else if clip_response.clicked() {
            action = ArrangeAction::SelectClip { track_idx, clip_id };
        }

        (Some(clip_rect), action)
    }

    fn draw_audio_clip(
        &self,
        painter: &egui::Painter,
        ui: &mut Ui,
        clip: &hallucinator_core::AudioClip,
        track_idx: usize,
        track_y: f32,
        ctx: &ArrangeContext,
        selected_clip: Option<(usize, ClipId)>,
    ) -> ArrangeAction {
        let (clip_rect, action) = self.draw_clip_base(
            painter, ui, clip.id, &clip.name,
            clip.start_sample, clip.end_sample(),
            track_idx, track_y, ctx, selected_clip,
            Color32::from_rgb(60, 100, 140),
            Color32::from_rgb(80, 130, 180),
            Color32::from_rgb(100, 150, 200),
        );
        if let Some(r) = clip_rect {
            self.draw_waveform(painter, r, &clip.samples, clip.channels as usize);
        }
        action
    }

    fn draw_midi_clip(
        &self,
        painter: &egui::Painter,
        ui: &mut Ui,
        clip: &hallucinator_core::MidiClip,
        track_idx: usize,
        track_y: f32,
        ctx: &ArrangeContext,
        selected_clip: Option<(usize, ClipId)>,
    ) -> ArrangeAction {
        let (clip_rect, action) = self.draw_clip_base(
            painter, ui, clip.id, &clip.name,
            clip.start_sample, clip.end_sample(),
            track_idx, track_y, ctx, selected_clip,
            Color32::from_rgb(60, 120, 60),
            Color32::from_rgb(80, 160, 80),
            Color32::from_rgb(100, 180, 100),
        );
        if let Some(r) = clip_rect {
            self.draw_note_preview(painter, r, clip);
        }
        action
    }

    fn draw_recording_preview(
        &self,
        painter: &egui::Painter,
        preview: &RecordingPreview,
        track_y: f32,
        ctx: &ArrangeContext,
    ) {
        if preview.samples.is_empty() {
            return;
        }

        let clip_start_beat = preview.start_sample as f64 / ctx.samples_per_beat;
        let clip_duration_beats = preview.samples.len() as f64 / ctx.samples_per_beat;

        let clip_x = ctx.rect.left() + ((clip_start_beat as f32 - ctx.start_beat) * self.pixels_per_beat);
        let clip_width = (clip_duration_beats as f32 * self.pixels_per_beat).max(2.0);

        let clip_rect = Rect::from_min_size(
            egui::pos2(clip_x, track_y + 4.0),
            Vec2::new(clip_width, self.track_height - 8.0),
        );

        painter.rect_filled(clip_rect, 4.0, Color32::from_rgb(140, 60, 60));
        painter.rect_stroke(clip_rect, 4.0, Stroke::new(1.0, Color32::from_rgb(200, 80, 80)), egui::StrokeKind::Outside);

        painter.text(
            egui::pos2(clip_rect.left() + 4.0, clip_rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            "Recording...",
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );

        self.draw_waveform(painter, clip_rect, &preview.samples, 1);
    }

    pub(super) fn draw_loop_region(
        &self,
        painter: &egui::Painter,
        ctx: &ArrangeContext,
        loop_start: u64,
        loop_end: u64,
    ) {
        let loop_start_beat = loop_start as f64 / ctx.samples_per_beat;
        let loop_end_beat = loop_end as f64 / ctx.samples_per_beat;

        let loop_x_start = ctx.rect.left() + ((loop_start_beat as f32 - ctx.start_beat) * self.pixels_per_beat);
        let loop_x_end = ctx.rect.left() + ((loop_end_beat as f32 - ctx.start_beat) * self.pixels_per_beat);

        // Fill in ruler area
        let loop_rect = Rect::from_min_max(
            egui::pos2(loop_x_start, ctx.ruler_rect.top()),
            egui::pos2(loop_x_end, ctx.ruler_rect.bottom()),
        );
        painter.rect_filled(loop_rect, 0.0, Color32::from_rgba_unmultiplied(100, 150, 200, 60));

        // Vertical bracket lines
        let bracket_color = Color32::from_rgb(100, 150, 200);
        painter.line_segment(
            [egui::pos2(loop_x_start, ctx.rect.top()), egui::pos2(loop_x_start, ctx.rect.bottom())],
            Stroke::new(1.0, bracket_color),
        );
        painter.line_segment(
            [egui::pos2(loop_x_end, ctx.rect.top()), egui::pos2(loop_x_end, ctx.rect.bottom())],
            Stroke::new(1.0, bracket_color),
        );

        // Edge handles (small rectangles at top of each bracket)
        let handle_color = Color32::from_rgb(130, 180, 230);
        let handle_width = 4.0;
        let handle_height = ctx.ruler_rect.height();

        // Start handle
        let start_handle = Rect::from_min_size(
            egui::pos2(loop_x_start - handle_width / 2.0, ctx.ruler_rect.top()),
            Vec2::new(handle_width, handle_height),
        );
        painter.rect_filled(start_handle, 2.0, handle_color);

        // End handle
        let end_handle = Rect::from_min_size(
            egui::pos2(loop_x_end - handle_width / 2.0, ctx.ruler_rect.top()),
            Vec2::new(handle_width, handle_height),
        );
        painter.rect_filled(end_handle, 2.0, handle_color);
    }

    pub(super) fn draw_playhead(
        &self,
        painter: &egui::Painter,
        ctx: &ArrangeContext,
        position_samples: u64,
    ) {
        let position_beat = position_samples as f64 / ctx.samples_per_beat;
        let playhead_x = ctx.rect.left() + ((position_beat as f32 - ctx.start_beat) * self.pixels_per_beat);

        if playhead_x < ctx.rect.left() || playhead_x > ctx.rect.right() {
            return;
        }

        let color = Color32::from_rgb(255, 100, 100);
        painter.line_segment(
            [egui::pos2(playhead_x, ctx.rect.top()), egui::pos2(playhead_x, ctx.rect.bottom())],
            Stroke::new(2.0, color),
        );

        let triangle = vec![
            egui::pos2(playhead_x, ctx.ruler_rect.bottom() - 4.0),
            egui::pos2(playhead_x - 6.0, ctx.ruler_rect.bottom() + 4.0),
            egui::pos2(playhead_x + 6.0, ctx.ruler_rect.bottom() + 4.0),
        ];
        painter.add(egui::Shape::convex_polygon(triangle, color, Stroke::NONE));
    }

    pub(super) fn draw_loop_selection_overlay(
        &self,
        painter: &egui::Painter,
        ctx: &ArrangeContext,
    ) {
        let Some((sel_start, sel_end)) = self.loop_selection else { return };

        let sel_x_start = ctx.rect.left() + ((sel_start - ctx.start_beat) * self.pixels_per_beat);
        let sel_x_end = ctx.rect.left() + ((sel_end - ctx.start_beat) * self.pixels_per_beat);
        let sel_rect = Rect::from_min_max(
            egui::pos2(sel_x_start, ctx.ruler_rect.top()),
            egui::pos2(sel_x_end, ctx.rect.bottom()),
        );
        painter.rect_filled(sel_rect, 0.0, Color32::from_rgba_unmultiplied(100, 180, 255, 40));
        painter.rect_stroke(sel_rect, 0.0, Stroke::new(1.0, Color32::from_rgb(100, 180, 255)), egui::StrokeKind::Inside);
    }

    fn draw_waveform(&self, painter: &egui::Painter, rect: Rect, samples: &[f32], channels: usize) {
        if samples.is_empty() || rect.width() < 4.0 {
            return;
        }

        let waveform_rect = Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + 16.0),
            rect.max,
        );

        let width = waveform_rect.width() as usize;
        let center_y = waveform_rect.center().y;
        let amplitude = waveform_rect.height() / 2.0 - 2.0;

        let total_frames = samples.len() / channels.max(1);
        let samples_per_pixel = total_frames / width.max(1);

        if samples_per_pixel == 0 {
            return;
        }

        for px in 0..width {
            let start = px * samples_per_pixel * channels;
            let end = ((px + 1) * samples_per_pixel * channels).min(samples.len());

            if start >= samples.len() {
                break;
            }

            let max_val = samples[start..end]
                .chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .map(|s| s.abs())
                .fold(0.0f32, f32::max);

            let x = waveform_rect.left() + px as f32;
            let h = max_val * amplitude;

            painter.rect_filled(
                Rect::from_center_size(egui::pos2(x, center_y), Vec2::new(1.0, h * 2.0)),
                0.0,
                Color32::from_rgba_unmultiplied(150, 200, 255, 120),
            );
        }
    }

    fn draw_note_preview(&self, painter: &egui::Painter, rect: Rect, clip: &hallucinator_core::MidiClip) {
        let preview_rect = Rect::from_min_max(
            egui::pos2(rect.left() + 2.0, rect.top() + 16.0),
            egui::pos2(rect.right() - 2.0, rect.bottom() - 2.0),
        );

        if clip.notes.is_empty() || preview_rect.width() < 4.0 {
            return;
        }

        let min_pitch = clip.notes.iter().map(|n| n.pitch).min().unwrap_or(60);
        let max_pitch = clip.notes.iter().map(|n| n.pitch).max().unwrap_or(72);
        let pitch_range = (max_pitch - min_pitch).max(12) as f32;
        let clip_duration_ticks = clip.length_samples as f32;

        for note in &clip.notes {
            let x = preview_rect.left() + (note.start_tick as f32 / clip_duration_ticks) * preview_rect.width();
            let w = (note.duration_ticks as f32 / clip_duration_ticks) * preview_rect.width();
            let y = preview_rect.bottom() - ((note.pitch - min_pitch) as f32 / pitch_range) * preview_rect.height();

            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(x, y - 2.0),
                    Vec2::new(w.max(2.0), 3.0),
                ),
                1.0,
                Color32::from_rgba_unmultiplied(200, 255, 200, 180),
            );
        }
    }
}
