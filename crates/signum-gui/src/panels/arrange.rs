//! Arrange panel - timeline grid with clips in bars:beats

use std::sync::atomic::Ordering;
use std::sync::Arc;

use egui::{Color32, Rect, Sense, Stroke, Ui, Vec2};
use signum_core::{ClipId, Track, TrackKind};
use signum_services::{AudioEngine, EngineState};

use super::timeline::RecordingPreview;

/// Action returned from arrange panel
#[derive(Clone)]
pub enum ArrangeAction {
    None,
    SelectClip { track_idx: usize, clip_id: ClipId },
    OpenClipEditor { track_idx: usize, clip_id: ClipId },
    Seek(u64),
    AddAudioTrack,
    AddMidiTrack,
    TogglePlayback,
}

/// Arrange panel state
pub struct ArrangePanel {
    pub pixels_per_beat: f32,
    pub scroll_offset_beats: f32,
    pub track_height: f32,
    pub vertical_scroll: f32,
    pub snap_to_grid: bool,
}

impl ArrangePanel {
    pub fn new() -> Self {
        Self {
            pixels_per_beat: 40.0,
            scroll_offset_beats: 0.0,
            track_height: 80.0,
            vertical_scroll: 0.0,
            snap_to_grid: true, // Default on
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        engine: &AudioEngine,
        state: &Arc<EngineState>,
        selected_track_idx: Option<usize>,
        selected_clip: Option<(usize, ClipId)>,
        recording_preview: Option<RecordingPreview>,
    ) -> ArrangeAction {
        let mut action = ArrangeAction::None;

        // Timeline area - takes full available space
        let available_rect = ui.available_rect_before_wrap();
        let (response, painter) = ui.allocate_painter(available_rect.size(), Sense::click_and_drag());
        let rect = response.rect;

        // Get timeline data
        let Ok(timeline) = state.timeline.lock() else {
            return action;
        };

        let sample_rate = timeline.transport.sample_rate as f64;
        let bpm = timeline.transport.bpm;
        let time_sig_num = timeline.transport.time_sig_num;
        let samples_per_beat = sample_rate * 60.0 / bpm;

        let ruler_height = 24.0;
        let track_area_top = rect.top() + ruler_height;

        // Calculate visible beat range
        let beats_visible = rect.width() / self.pixels_per_beat;
        let start_beat = self.scroll_offset_beats;
        let end_beat = start_beat + beats_visible;

        // === LAYER 1: Track backgrounds (bottom layer) ===
        for (track_idx, _track) in timeline.tracks.iter().enumerate() {
            let track_y = track_area_top + (track_idx as f32 * self.track_height) - self.vertical_scroll;

            if track_y + self.track_height < track_area_top || track_y > rect.bottom() {
                continue;
            }

            let track_rect = Rect::from_min_size(
                egui::pos2(rect.left(), track_y),
                Vec2::new(rect.width(), self.track_height),
            );

            // Selected track is darker, non-selected tracks are lighter
            let is_selected = selected_track_idx == Some(track_idx);
            let bg_color = if is_selected {
                // Selected track - darker
                if track_idx % 2 == 0 {
                    Color32::from_gray(28)
                } else {
                    Color32::from_gray(32)
                }
            } else {
                // Non-selected tracks - lighter
                if track_idx % 2 == 0 {
                    Color32::from_gray(42)
                } else {
                    Color32::from_gray(48)
                }
            };
            painter.rect_filled(track_rect, 0.0, bg_color);

            // Track lane separator line
            painter.line_segment(
                [egui::pos2(rect.left(), track_y + self.track_height), egui::pos2(rect.right(), track_y + self.track_height)],
                Stroke::new(1.0, Color32::from_gray(25)),
            );
        }

        // Fill remaining area below tracks (lighter)
        let tracks_bottom = track_area_top + (timeline.tracks.len() as f32 * self.track_height);
        if tracks_bottom < rect.bottom() {
            let empty_rect = Rect::from_min_max(
                egui::pos2(rect.left(), tracks_bottom),
                rect.max,
            );
            painter.rect_filled(empty_rect, 0.0, Color32::from_gray(50));
        }

        // === LAYER 2: Grid lines (on top of track backgrounds) ===
        // Dynamic grid subdivision based on zoom level
        // Higher pixels_per_beat = more zoomed in = finer subdivisions
        let subdivision = if self.pixels_per_beat >= 160.0 {
            8.0  // 32nd notes (1/8 beat)
        } else if self.pixels_per_beat >= 80.0 {
            4.0  // 16th notes (1/4 beat)
        } else if self.pixels_per_beat >= 40.0 {
            2.0  // 8th notes (1/2 beat)
        } else if self.pixels_per_beat >= 20.0 {
            1.0  // quarter notes (1 beat)
        } else {
            0.0  // bars only
        };

        let grid_step = if subdivision > 0.0 { 1.0 / subdivision } else { time_sig_num as f32 };
        let pixels_per_grid = self.pixels_per_beat * grid_step;

        // Only draw if grid lines have minimum spacing
        if pixels_per_grid >= 8.0 {
            let mut pos = (start_beat / grid_step).floor() * grid_step;
            while pos <= end_beat {
                let x = rect.left() + ((pos - start_beat) * self.pixels_per_beat);

                // Determine line type based on position
                let beat_pos = pos;
                let is_bar = (beat_pos.round() as u32) % (time_sig_num as u32) == 0
                    && (beat_pos - beat_pos.round()).abs() < 0.001;
                let is_beat = (beat_pos - beat_pos.round()).abs() < 0.001;

                let stroke = if is_bar {
                    Stroke::new(1.0, Color32::from_gray(80))  // Bar line
                } else if is_beat {
                    Stroke::new(0.5, Color32::from_gray(60))  // Beat line
                } else {
                    Stroke::new(0.5, Color32::from_gray(45))  // Subdivision line
                };

                painter.line_segment(
                    [egui::pos2(x, track_area_top), egui::pos2(x, rect.bottom())],
                    stroke,
                );

                pos += grid_step;
            }
        }

        // === LAYER 3: Ruler (on top) ===
        let ruler_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), ruler_height));
        painter.rect_filled(ruler_rect, 0.0, Color32::from_gray(45));

        // Ruler tick marks with dynamic subdivision
        if pixels_per_grid >= 8.0 {
            let mut pos = (start_beat / grid_step).floor() * grid_step;
            while pos <= end_beat {
                let x = rect.left() + ((pos - start_beat) * self.pixels_per_beat);

                let beat_pos = pos;
                let is_bar = (beat_pos.round() as u32) % (time_sig_num as u32) == 0
                    && (beat_pos - beat_pos.round()).abs() < 0.001;
                let is_beat = (beat_pos - beat_pos.round()).abs() < 0.001;

                // Tick heights: bar > beat > subdivision
                let tick_height = if is_bar { 10.0 } else if is_beat { 6.0 } else { 3.0 };
                let tick_color = if is_bar || is_beat {
                    Color32::from_gray(120)
                } else {
                    Color32::from_gray(80)
                };

                painter.line_segment(
                    [egui::pos2(x, ruler_rect.bottom() - tick_height), egui::pos2(x, ruler_rect.bottom())],
                    Stroke::new(1.0, tick_color),
                );

                // Bar numbers at bar lines
                if is_bar {
                    let bar = (beat_pos.round() as u32) / (time_sig_num as u32) + 1;
                    painter.text(
                        egui::pos2(x + 4.0, ruler_rect.top() + 4.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", bar),
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }

                pos += grid_step;
            }
        }

        // Ruler bottom border
        painter.line_segment(
            [egui::pos2(rect.left(), ruler_rect.bottom()), egui::pos2(rect.right(), ruler_rect.bottom())],
            Stroke::new(1.0, Color32::from_gray(60)),
        );

        // === LAYER 4: Clips (on top of grid) ===
        for (track_idx, track) in timeline.tracks.iter().enumerate() {
            let track_y = track_area_top + (track_idx as f32 * self.track_height) - self.vertical_scroll;

            if track_y + self.track_height < track_area_top || track_y > rect.bottom() {
                continue;
            }

            // Draw audio clips
            for clip in &track.clips {
                let clip_action = self.draw_audio_clip(
                    &painter,
                    ui,
                    clip,
                    track_idx,
                    track_y,
                    start_beat,
                    samples_per_beat,
                    rect,
                    selected_clip,
                );
                if !matches!(clip_action, ArrangeAction::None) {
                    action = clip_action;
                }
            }

            // Draw MIDI clips
            for clip in &track.midi_clips {
                let clip_action = self.draw_midi_clip(
                    &painter,
                    ui,
                    clip,
                    track_idx,
                    track_y,
                    start_beat,
                    samples_per_beat,
                    rect,
                    selected_clip,
                );
                if !matches!(clip_action, ArrangeAction::None) {
                    action = clip_action;
                }
            }

            // Draw recording preview on armed track
            if track.armed {
                if let Some(ref preview) = recording_preview {
                    self.draw_recording_preview(&painter, preview, track_y, start_beat, samples_per_beat, rect);
                }
            }
        }

        // Draw loop region if enabled
        if timeline.transport.loop_enabled {
            let loop_start_beat = timeline.transport.loop_start as f64 / samples_per_beat;
            let loop_end_beat = timeline.transport.loop_end as f64 / samples_per_beat;

            let loop_x_start = rect.left() + ((loop_start_beat as f32 - start_beat) * self.pixels_per_beat);
            let loop_x_end = rect.left() + ((loop_end_beat as f32 - start_beat) * self.pixels_per_beat);

            // Loop region highlight
            let loop_rect = Rect::from_min_max(
                egui::pos2(loop_x_start, ruler_rect.top()),
                egui::pos2(loop_x_end, ruler_rect.bottom()),
            );
            painter.rect_filled(loop_rect, 0.0, Color32::from_rgba_unmultiplied(100, 150, 200, 60));

            // Loop bracket lines
            painter.line_segment(
                [egui::pos2(loop_x_start, rect.top()), egui::pos2(loop_x_start, rect.bottom())],
                Stroke::new(1.0, Color32::from_rgb(100, 150, 200)),
            );
            painter.line_segment(
                [egui::pos2(loop_x_end, rect.top()), egui::pos2(loop_x_end, rect.bottom())],
                Stroke::new(1.0, Color32::from_rgb(100, 150, 200)),
            );
        }

        drop(timeline);

        // Draw playhead
        let position_samples = state.position.load(Ordering::SeqCst);
        let position_beat = position_samples as f64 / samples_per_beat;
        let playhead_x = rect.left() + ((position_beat as f32 - start_beat) * self.pixels_per_beat);

        if playhead_x >= rect.left() && playhead_x <= rect.right() {
            painter.line_segment(
                [egui::pos2(playhead_x, rect.top()), egui::pos2(playhead_x, rect.bottom())],
                Stroke::new(2.0, Color32::from_rgb(255, 100, 100)),
            );

            // Playhead triangle at top
            let triangle = vec![
                egui::pos2(playhead_x, ruler_rect.bottom() - 4.0),
                egui::pos2(playhead_x - 6.0, ruler_rect.bottom() + 4.0),
                egui::pos2(playhead_x + 6.0, ruler_rect.bottom() + 4.0),
            ];
            painter.add(egui::Shape::convex_polygon(
                triangle,
                Color32::from_rgb(255, 100, 100),
                Stroke::NONE,
            ));
        }

        // Handle click to seek
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let mut click_beat = start_beat + (pos.x - rect.left()) / self.pixels_per_beat;

                // Apply snap to grid at current subdivision level
                if self.snap_to_grid {
                    click_beat = (click_beat / grid_step).round() * grid_step;
                }

                let click_samples = (click_beat as f64 * samples_per_beat) as u64;
                action = ArrangeAction::Seek(click_samples);
            }
        }

        // Handle spacebar to toggle playback (when hovering over arrange panel)
        if response.hovered() {
            ui.input(|i| {
                if i.key_pressed(egui::Key::Space) {
                    action = ArrangeAction::TogglePlayback;
                }
            });
        }

        // Handle right-click context menu on empty area
        response.context_menu(|ui| {
            if ui.button("Add Audio Track").clicked() {
                action = ArrangeAction::AddAudioTrack;
                ui.close_menu();
            }
            if ui.button("Add MIDI Track").clicked() {
                action = ArrangeAction::AddMidiTrack;
                ui.close_menu();
            }
        });

        // Handle scroll - horizontal for panning, vertical with ctrl/cmd for zoom
        // Check if pointer is within our rect for scroll handling
        let pointer_in_rect = ui.ctx().input(|i| {
            i.pointer.hover_pos().map(|p| rect.contains(p)).unwrap_or(false)
        });

        if pointer_in_rect {
            // Check for mouse wheel events in raw events
            let (scroll_y, modifiers) = ui.ctx().input(|i| {
                let mut scroll = 0.0_f32;
                for event in &i.events {
                    if let egui::Event::MouseWheel { delta, .. } = event {
                        scroll += delta.y;
                    }
                }
                // Also check smooth/raw scroll delta
                scroll += i.smooth_scroll_delta.y + i.raw_scroll_delta.y;
                (scroll, i.modifiers)
            });

            if scroll_y.abs() > 0.1 {
                if modifiers.ctrl || modifiers.command {
                    // Ctrl/Cmd + scroll for zoom
                    let zoom_factor = 1.0 + scroll_y * 0.02;
                    self.pixels_per_beat = (self.pixels_per_beat * zoom_factor).clamp(10.0, 200.0);
                } else {
                    // Vertical scroll pans horizontally (timeline convention)
                    self.scroll_offset_beats = (self.scroll_offset_beats - scroll_y / self.pixels_per_beat).max(0.0);
                }
            }

            // Also handle horizontal scroll for panning
            let scroll_x = ui.ctx().input(|i| i.smooth_scroll_delta.x + i.raw_scroll_delta.x);
            if scroll_x.abs() > 0.1 {
                self.scroll_offset_beats = (self.scroll_offset_beats - scroll_x / self.pixels_per_beat).max(0.0);
            }
        }

        action
    }

    fn draw_audio_clip(
        &self,
        painter: &egui::Painter,
        ui: &mut Ui,
        clip: &signum_core::AudioClip,
        track_idx: usize,
        track_y: f32,
        start_beat: f32,
        samples_per_beat: f64,
        rect: Rect,
    selected_clip: Option<(usize, ClipId)>,
    ) -> ArrangeAction {
        let mut action = ArrangeAction::None;

        let clip_start_beat = clip.start_sample as f64 / samples_per_beat;
        let clip_end_beat = clip.end_sample() as f64 / samples_per_beat;
        let beats_visible = rect.width() / self.pixels_per_beat;
        let end_beat = start_beat + beats_visible;

        // Skip if not visible
        if clip_end_beat < start_beat as f64 || clip_start_beat > end_beat as f64 {
            return action;
        }

        let clip_x = rect.left() + ((clip_start_beat as f32 - start_beat) * self.pixels_per_beat);
        let clip_width = (clip_end_beat - clip_start_beat) as f32 * self.pixels_per_beat;

        let clip_rect = Rect::from_min_size(
            egui::pos2(clip_x, track_y + 4.0),
            Vec2::new(clip_width, self.track_height - 8.0),
        );

        // Audio clips are blue
        let is_selected = selected_clip == Some((track_idx, clip.id));
        let fill_color = if is_selected {
            Color32::from_rgb(80, 130, 180)
        } else {
            Color32::from_rgb(60, 100, 140)
        };

        painter.rect_filled(clip_rect, 4.0, fill_color);
        painter.rect_stroke(
            clip_rect,
            4.0,
            Stroke::new(if is_selected { 2.0 } else { 1.0 }, Color32::from_rgb(100, 150, 200)),
            egui::StrokeKind::Outside,
        );

        // Clip name
        painter.text(
            egui::pos2(clip_rect.left() + 4.0, clip_rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            &clip.name,
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );

        // Draw waveform
        self.draw_waveform(painter, clip_rect, &clip.samples, clip.channels as usize);

        // Handle clicks
        let clip_response = ui.allocate_rect(clip_rect, Sense::click());
        if clip_response.clicked() {
            action = ArrangeAction::SelectClip {
                track_idx,
                clip_id: clip.id,
            };
        }
        if clip_response.double_clicked() {
            action = ArrangeAction::OpenClipEditor {
                track_idx,
                clip_id: clip.id,
            };
        }

        action
    }

    fn draw_midi_clip(
        &self,
        painter: &egui::Painter,
        ui: &mut Ui,
        clip: &signum_core::MidiClip,
        track_idx: usize,
        track_y: f32,
        start_beat: f32,
        samples_per_beat: f64,
        rect: Rect,
        selected_clip: Option<(usize, ClipId)>,
    ) -> ArrangeAction {
        let mut action = ArrangeAction::None;

        let clip_start_beat = clip.start_sample as f64 / samples_per_beat;
        let clip_end_beat = clip.end_sample() as f64 / samples_per_beat;
        let beats_visible = rect.width() / self.pixels_per_beat;
        let end_beat = start_beat + beats_visible;

        // Skip if not visible
        if clip_end_beat < start_beat as f64 || clip_start_beat > end_beat as f64 {
            return action;
        }

        let clip_x = rect.left() + ((clip_start_beat as f32 - start_beat) * self.pixels_per_beat);
        let clip_width = (clip_end_beat - clip_start_beat) as f32 * self.pixels_per_beat;

        let clip_rect = Rect::from_min_size(
            egui::pos2(clip_x, track_y + 4.0),
            Vec2::new(clip_width, self.track_height - 8.0),
        );

        // MIDI clips are green
        let is_selected = selected_clip == Some((track_idx, clip.id));
        let fill_color = if is_selected {
            Color32::from_rgb(80, 160, 80)
        } else {
            Color32::from_rgb(60, 120, 60)
        };

        painter.rect_filled(clip_rect, 4.0, fill_color);
        painter.rect_stroke(
            clip_rect,
            4.0,
            Stroke::new(if is_selected { 2.0 } else { 1.0 }, Color32::from_rgb(100, 180, 100)),
            egui::StrokeKind::Outside,
        );

        // Clip name
        painter.text(
            egui::pos2(clip_rect.left() + 4.0, clip_rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            &clip.name,
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );

        // Draw note density preview (simplified visualization)
        self.draw_note_preview(painter, clip_rect, clip);

        // Handle clicks
        let clip_response = ui.allocate_rect(clip_rect, Sense::click());
        if clip_response.clicked() {
            action = ArrangeAction::SelectClip {
                track_idx,
                clip_id: clip.id,
            };
        }
        if clip_response.double_clicked() {
            action = ArrangeAction::OpenClipEditor {
                track_idx,
                clip_id: clip.id,
            };
        }

        action
    }

    fn draw_recording_preview(
        &self,
        painter: &egui::Painter,
        preview: &RecordingPreview,
        track_y: f32,
        start_beat: f32,
        samples_per_beat: f64,
        rect: Rect,
    ) {
        if preview.samples.is_empty() {
            return;
        }

        let clip_start_beat = preview.start_sample as f64 / samples_per_beat;
        let clip_duration_samples = preview.samples.len() as f64;
        let clip_duration_beats = clip_duration_samples / samples_per_beat;

        let clip_x = rect.left() + ((clip_start_beat as f32 - start_beat) * self.pixels_per_beat);
        let clip_width = (clip_duration_beats as f32 * self.pixels_per_beat).max(2.0);

        let clip_rect = Rect::from_min_size(
            egui::pos2(clip_x, track_y + 4.0),
            Vec2::new(clip_width, self.track_height - 8.0),
        );

        // Recording clip with red tint
        painter.rect_filled(clip_rect, 4.0, Color32::from_rgb(140, 60, 60));
        painter.rect_stroke(
            clip_rect,
            4.0,
            Stroke::new(1.0, Color32::from_rgb(200, 80, 80)),
            egui::StrokeKind::Outside,
        );

        painter.text(
            egui::pos2(clip_rect.left() + 4.0, clip_rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            "Recording...",
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );

        self.draw_waveform(painter, clip_rect, &preview.samples, 1);
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

    fn draw_note_preview(&self, painter: &egui::Painter, rect: Rect, clip: &signum_core::MidiClip) {
        let preview_rect = Rect::from_min_max(
            egui::pos2(rect.left() + 2.0, rect.top() + 16.0),
            egui::pos2(rect.right() - 2.0, rect.bottom() - 2.0),
        );

        if clip.notes.is_empty() || preview_rect.width() < 4.0 {
            return;
        }

        // Find pitch range
        let min_pitch = clip.notes.iter().map(|n| n.pitch).min().unwrap_or(60);
        let max_pitch = clip.notes.iter().map(|n| n.pitch).max().unwrap_or(72);
        let pitch_range = (max_pitch - min_pitch).max(12) as f32;

        let clip_duration_ticks = clip.length_samples as f32; // Approximation

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

    /// Get vertical scroll offset for syncing with track headers
    pub fn vertical_scroll(&self) -> f32 {
        self.vertical_scroll
    }
}

impl Default for ArrangePanel {
    fn default() -> Self {
        Self::new()
    }
}
