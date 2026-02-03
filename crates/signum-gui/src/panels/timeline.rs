//! Timeline panel with tracks and clips

use std::sync::Arc;
use std::sync::atomic::Ordering;

use egui::{Color32, Rect, Sense, Stroke, Ui, Vec2};
use signum_services::{AudioEngine, EngineState};

/// Live recording preview data
pub struct RecordingPreview {
    pub samples: Vec<f32>,
    pub start_sample: u64,
    pub sample_rate: u32,
}

pub struct TimelinePanel {
    pixels_per_second: f32,
    scroll_offset: f32,
    track_height: f32,
}

impl TimelinePanel {
    pub fn new() -> Self {
        Self {
            pixels_per_second: 100.0,
            scroll_offset: 0.0,
            track_height: 80.0,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        engine: &AudioEngine,
        state: &Arc<EngineState>,
        recording_preview: Option<RecordingPreview>,
    ) {
        // Zoom controls
        ui.horizontal(|ui| {
            ui.label("Zoom:");
            if ui.button("-").clicked() {
                self.pixels_per_second = (self.pixels_per_second * 0.8).max(10.0);
            }
            if ui.button("+").clicked() {
                self.pixels_per_second = (self.pixels_per_second * 1.25).min(500.0);
            }
            ui.label(format!("{:.0} px/s", self.pixels_per_second));

            ui.separator();
            ui.label("Drop WAV files to import");
        });

        ui.separator();

        // Timeline area
        let available_rect = ui.available_rect_before_wrap();
        let (response, painter) = ui.allocate_painter(available_rect.size(), Sense::click_and_drag());

        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 0.0, Color32::from_gray(30));

        // Get timeline data
        let Ok(timeline) = state.timeline.lock() else {
            return;
        };

        let sample_rate = timeline.transport.sample_rate as f64;

        // Draw time ruler
        let ruler_height = 20.0;
        let ruler_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), ruler_height));
        painter.rect_filled(ruler_rect, 0.0, Color32::from_gray(40));

        // Draw time markers
        let seconds_visible = rect.width() / self.pixels_per_second;
        let start_sec = self.scroll_offset;
        let end_sec = start_sec + seconds_visible;

        let mut t = start_sec.floor();
        while t <= end_sec {
            let x = rect.left() + (t - start_sec) * self.pixels_per_second;
            painter.line_segment(
                [egui::pos2(x, ruler_rect.top()), egui::pos2(x, ruler_rect.bottom())],
                Stroke::new(1.0, Color32::from_gray(80)),
            );

            let mins = (t / 60.0) as i32;
            let secs = t % 60.0;
            painter.text(
                egui::pos2(x + 2.0, ruler_rect.top() + 2.0),
                egui::Align2::LEFT_TOP,
                format!("{}:{:02.0}", mins, secs),
                egui::FontId::proportional(10.0),
                Color32::from_gray(150),
            );

            t += 1.0;
        }

        // Draw tracks
        let track_area_top = rect.top() + ruler_height;

        for (track_idx, track) in timeline.tracks.iter().enumerate() {
            let track_y = track_area_top + (track_idx as f32 * self.track_height);
            let track_rect = Rect::from_min_size(
                egui::pos2(rect.left(), track_y),
                Vec2::new(rect.width(), self.track_height),
            );

            // Track background
            let bg_color = if track_idx % 2 == 0 {
                Color32::from_gray(35)
            } else {
                Color32::from_gray(40)
            };
            painter.rect_filled(track_rect, 0.0, bg_color);

            // Track label
            painter.text(
                egui::pos2(rect.left() + 5.0, track_y + 2.0),
                egui::Align2::LEFT_TOP,
                &track.name,
                egui::FontId::proportional(12.0),
                Color32::from_gray(180),
            );

            // Draw existing clips
            for clip in &track.clips {
                let clip_start_sec = clip.start_sample as f64 / sample_rate;
                let clip_end_sec = clip.end_sample() as f64 / sample_rate;

                if clip_end_sec < start_sec as f64 || clip_start_sec > end_sec as f64 {
                    continue;
                }

                let clip_x = rect.left() + ((clip_start_sec as f32 - start_sec) * self.pixels_per_second);
                let clip_width = (clip_end_sec - clip_start_sec) as f32 * self.pixels_per_second;

                let clip_rect = Rect::from_min_size(
                    egui::pos2(clip_x, track_y + 18.0),
                    Vec2::new(clip_width, self.track_height - 22.0),
                );

                painter.rect_filled(clip_rect, 4.0, Color32::from_rgb(60, 100, 140));
                painter.rect_stroke(clip_rect, 4.0, Stroke::new(1.0, Color32::from_rgb(80, 130, 180)), egui::StrokeKind::Outside);

                painter.text(
                    egui::pos2(clip_rect.left() + 4.0, clip_rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    &clip.name,
                    egui::FontId::proportional(10.0),
                    Color32::WHITE,
                );

                self.draw_waveform(&painter, clip_rect, &clip.samples, clip.channels as usize);
            }

            // Draw live recording preview on first track
            if track_idx == 0 {
                if let Some(ref preview) = recording_preview {
                    self.draw_recording_preview(&painter, preview, track_y, start_sec, sample_rate, &rect);
                }
            }
        }

        drop(timeline);

        // Draw playhead
        let position_samples = state.position.load(Ordering::SeqCst);
        let position_sec = position_samples as f64 / sample_rate;
        let playhead_x = rect.left() + ((position_sec as f32 - start_sec) * self.pixels_per_second);

        if playhead_x >= rect.left() && playhead_x <= rect.right() {
            painter.line_segment(
                [egui::pos2(playhead_x, rect.top()), egui::pos2(playhead_x, rect.bottom())],
                Stroke::new(2.0, Color32::from_rgb(255, 100, 100)),
            );
        }

        // Handle click to seek
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let click_sec = start_sec + (pos.x - rect.left()) / self.pixels_per_second;
                let click_samples = (click_sec as f64 * sample_rate) as u64;
                engine.seek(click_samples);
            }
        }

        // Handle scroll
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.x);
        if scroll_delta.abs() > 0.0 {
            self.scroll_offset = (self.scroll_offset - scroll_delta / self.pixels_per_second).max(0.0);
        }
    }

    fn draw_recording_preview(
        &self,
        painter: &egui::Painter,
        preview: &RecordingPreview,
        track_y: f32,
        start_sec: f32,
        sample_rate: f64,
        rect: &Rect,
    ) {
        if preview.samples.is_empty() {
            return;
        }

        let clip_start_sec = preview.start_sample as f64 / sample_rate;
        let clip_duration_sec = preview.samples.len() as f64 / preview.sample_rate as f64;

        let clip_x = rect.left() + ((clip_start_sec as f32 - start_sec) * self.pixels_per_second);
        let clip_width = clip_duration_sec as f32 * self.pixels_per_second;

        let clip_rect = Rect::from_min_size(
            egui::pos2(clip_x, track_y + 18.0),
            Vec2::new(clip_width.max(2.0), self.track_height - 22.0),
        );

        // Recording clip with red tint
        painter.rect_filled(clip_rect, 4.0, Color32::from_rgb(140, 60, 60));
        painter.rect_stroke(clip_rect, 4.0, Stroke::new(1.0, Color32::from_rgb(200, 80, 80)), egui::StrokeKind::Outside);

        painter.text(
            egui::pos2(clip_rect.left() + 4.0, clip_rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            "Recording...",
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );

        // Draw waveform (mono, channels=1)
        self.draw_waveform(painter, clip_rect, &preview.samples, 1);
    }

    fn draw_waveform(&self, painter: &egui::Painter, rect: Rect, samples: &[f32], channels: usize) {
        if samples.is_empty() || rect.width() < 4.0 {
            return;
        }

        let width = rect.width() as usize;
        let center_y = rect.center().y;
        let amplitude = rect.height() / 2.0 - 2.0;

        let total_frames = samples.len() / channels.max(1);
        let samples_per_pixel = total_frames / width.max(1);

        if samples_per_pixel == 0 {
            // Very short clip - just draw what we have
            for (i, chunk) in samples.chunks(channels).enumerate() {
                let x = rect.left() + i as f32;
                if x > rect.right() {
                    break;
                }
                let val: f32 = chunk.iter().sum::<f32>() / channels as f32;
                let h = val.abs() * amplitude;
                painter.rect_filled(
                    Rect::from_center_size(egui::pos2(x, center_y), Vec2::new(1.0, h * 2.0)),
                    0.0,
                    Color32::from_rgba_unmultiplied(100, 180, 220, 150),
                );
            }
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

            let x = rect.left() + px as f32;
            let h = max_val * amplitude;

            painter.rect_filled(
                Rect::from_center_size(egui::pos2(x, center_y), Vec2::new(1.0, h * 2.0)),
                0.0,
                Color32::from_rgba_unmultiplied(100, 180, 220, 150),
            );
        }
    }
}
