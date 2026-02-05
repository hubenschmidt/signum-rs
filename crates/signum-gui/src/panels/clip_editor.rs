//! Clip editor panel - combines piano roll (MIDI) and waveform editor (audio)

use crate::clipboard::DawClipboard;
use egui::{Color32, Rect, Sense, Stroke, Ui, Vec2};
use signum_core::{AudioClip, ClipId, MidiClip};

use super::piano_roll::{PianoRollAction, PianoRollPanel};

/// The type of clip being edited
pub enum ClipType {
    Audio,
    Midi,
}

/// Clip editor panel state
pub struct ClipEditorPanel {
    piano_roll: PianoRollPanel,
    // Audio editor state
    audio_zoom: f32,
    audio_scroll: f32,
}

impl ClipEditorPanel {
    pub fn new() -> Self {
        Self {
            piano_roll: PianoRollPanel::new(),
            audio_zoom: 1.0,
            audio_scroll: 0.0,
        }
    }

    /// Render UI for MIDI clip (piano roll)
    pub fn ui_midi(
        &mut self,
        ui: &mut Ui,
        clip: &mut MidiClip,
        bpm: f64,
        sample_rate: u32,
        clip_start_sample: u64,
        playback_position: u64,
        clipboard: &DawClipboard,
    ) -> PianoRollAction {
        self.piano_roll.ui(ui, clip, bpm, sample_rate, clip_start_sample, playback_position, clipboard)
    }

    /// Render UI for audio clip (waveform editor)
    pub fn ui_audio(&mut self, ui: &mut Ui, clip: &AudioClip, sample_rate: u32) {
        // Toolbar
        ui.horizontal(|ui| {
            ui.label("Audio Editor");
            ui.separator();
            ui.label(&clip.name);
            ui.separator();

            if ui.button("-").clicked() {
                self.audio_zoom = (self.audio_zoom * 0.8).max(0.1);
            }
            if ui.button("+").clicked() {
                self.audio_zoom = (self.audio_zoom * 1.25).min(10.0);
            }
            ui.label(format!("{:.1}x", self.audio_zoom));
        });

        ui.separator();

        // Waveform display area
        let available = ui.available_rect_before_wrap();
        let (response, painter) = ui.allocate_painter(available.size(), Sense::click_and_drag());
        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 0.0, Color32::from_gray(25));

        // Draw time ruler
        let ruler_height = 20.0;
        let ruler_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), ruler_height));
        painter.rect_filled(ruler_rect, 0.0, Color32::from_gray(40));

        let waveform_rect = Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + ruler_height),
            rect.max,
        );

        // Calculate visible range
        let total_duration_secs = clip.length_samples as f32 / sample_rate as f32;
        let visible_duration = total_duration_secs / self.audio_zoom;
        let start_time = self.audio_scroll;
        let _end_time = start_time + visible_duration;

        // Draw time markers
        let seconds_per_pixel = visible_duration / rect.width();
        let marker_interval = Self::calculate_marker_interval(seconds_per_pixel);

        let mut t = (start_time / marker_interval).floor() * marker_interval;
        while t < start_time + visible_duration {
            let x = rect.left() + ((t - start_time) / visible_duration) * rect.width();
            if x >= rect.left() && x <= rect.right() {
                painter.line_segment(
                    [egui::pos2(x, ruler_rect.top()), egui::pos2(x, ruler_rect.bottom())],
                    Stroke::new(1.0, Color32::from_gray(80)),
                );

                let label = if t >= 60.0 {
                    format!("{}:{:05.2}", (t / 60.0) as u32, t % 60.0)
                } else {
                    format!("{:.2}s", t)
                };

                painter.text(
                    egui::pos2(x + 2.0, ruler_rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    egui::FontId::proportional(9.0),
                    Color32::from_gray(150),
                );
            }
            t += marker_interval;
        }

        // Draw waveform
        self.draw_waveform(&painter, waveform_rect, clip, start_time, visible_duration, sample_rate);

        // Draw center line
        let center_y = waveform_rect.center().y;
        painter.line_segment(
            [egui::pos2(waveform_rect.left(), center_y), egui::pos2(waveform_rect.right(), center_y)],
            Stroke::new(0.5, Color32::from_gray(60)),
        );

        // Handle scroll
        if response.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta);
            if scroll.x.abs() > 0.0 {
                self.audio_scroll = (self.audio_scroll - scroll.x * seconds_per_pixel * 10.0).max(0.0);
                self.audio_scroll = self.audio_scroll.min(total_duration_secs - visible_duration);
            }
        }
    }

    fn draw_waveform(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        clip: &AudioClip,
        start_time: f32,
        visible_duration: f32,
        sample_rate: u32,
    ) {
        if clip.samples.is_empty() || rect.width() < 4.0 {
            return;
        }

        let channels = clip.channels as usize;
        let total_frames = clip.samples.len() / channels.max(1);

        let start_sample = (start_time * sample_rate as f32) as usize;
        let visible_samples = (visible_duration * sample_rate as f32) as usize;
        let end_sample = (start_sample + visible_samples).min(total_frames);

        let width = rect.width() as usize;
        let center_y = rect.center().y;
        let amplitude = rect.height() / 2.0 - 4.0;

        let samples_per_pixel = visible_samples.max(1) / width.max(1);

        if samples_per_pixel == 0 {
            return;
        }

        for px in 0..width {
            let frame_start = start_sample + px * samples_per_pixel;
            let frame_end = (frame_start + samples_per_pixel).min(end_sample);

            if frame_start >= total_frames {
                break;
            }

            let sample_start = frame_start * channels;
            let sample_end = (frame_end * channels).min(clip.samples.len());

            if sample_start >= clip.samples.len() {
                break;
            }

            // Find min/max in this range
            let mut min_val = 0.0f32;
            let mut max_val = 0.0f32;

            for chunk in clip.samples[sample_start..sample_end].chunks(channels) {
                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                min_val = min_val.min(mono);
                max_val = max_val.max(mono);
            }

            let x = rect.left() + px as f32;
            let y_top = center_y - max_val * amplitude;
            let y_bottom = center_y - min_val * amplitude;

            painter.line_segment(
                [egui::pos2(x, y_top), egui::pos2(x, y_bottom)],
                Stroke::new(1.0, Color32::from_rgb(100, 180, 220)),
            );
        }
    }

    fn calculate_marker_interval(seconds_per_pixel: f32) -> f32 {
        let min_pixel_spacing = 80.0;
        let min_interval = seconds_per_pixel * min_pixel_spacing;

        // Round to nice intervals: 0.1, 0.5, 1, 5, 10, 30, 60...
        let intervals = [0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0];

        for &interval in &intervals {
            if interval >= min_interval {
                return interval;
            }
        }

        300.0
    }

    /// Get the piano roll for direct access
    pub fn piano_roll(&self) -> &PianoRollPanel {
        &self.piano_roll
    }

    /// Get mutable piano roll
    pub fn piano_roll_mut(&mut self) -> &mut PianoRollPanel {
        &mut self.piano_roll
    }
}

impl Default for ClipEditorPanel {
    fn default() -> Self {
        Self::new()
    }
}
