//! Drum roll panel for 808-style drum sequencing

use crate::clipboard::DawClipboard;
use egui::{Color32, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use hallucinator_core::{ClipId, MidiClip, MidiNote};
use hallucinator_services::{
    KICK, RIM_SHOT, SNARE, CLAP, CLOSED_HAT, OPEN_HAT, LOW_TOM, MID_TOM, HIGH_TOM,
    CRASH, COWBELL, HI_CONGA, MID_CONGA, LOW_CONGA, MARACAS, CLAVES,
};

/// Drum lane definition
struct DrumLane {
    name: &'static str,
    pitch: u8,
    color: Color32,
    key: Option<Key>, // Keyboard shortcut
}

const DRUM_LANES: [DrumLane; 16] = [
    DrumLane { name: "Kick", pitch: KICK, color: Color32::from_rgb(200, 80, 80), key: Some(Key::Z) },
    DrumLane { name: "Rim", pitch: RIM_SHOT, color: Color32::from_rgb(180, 100, 100), key: Some(Key::X) },
    DrumLane { name: "Snare", pitch: SNARE, color: Color32::from_rgb(200, 150, 80), key: Some(Key::C) },
    DrumLane { name: "Clap", pitch: CLAP, color: Color32::from_rgb(200, 200, 80), key: Some(Key::V) },
    DrumLane { name: "C.Hat", pitch: CLOSED_HAT, color: Color32::from_rgb(80, 200, 80), key: Some(Key::B) },
    DrumLane { name: "O.Hat", pitch: OPEN_HAT, color: Color32::from_rgb(80, 200, 150), key: Some(Key::N) },
    DrumLane { name: "L.Tom", pitch: LOW_TOM, color: Color32::from_rgb(80, 150, 200), key: Some(Key::A) },
    DrumLane { name: "M.Tom", pitch: MID_TOM, color: Color32::from_rgb(80, 80, 200), key: Some(Key::S) },
    DrumLane { name: "H.Tom", pitch: HIGH_TOM, color: Color32::from_rgb(150, 80, 200), key: Some(Key::D) },
    DrumLane { name: "Crash", pitch: CRASH, color: Color32::from_rgb(200, 200, 200), key: Some(Key::F) },
    DrumLane { name: "Cowbell", pitch: COWBELL, color: Color32::from_rgb(180, 140, 80), key: Some(Key::G) },
    DrumLane { name: "Hi Cga", pitch: HI_CONGA, color: Color32::from_rgb(180, 120, 60), key: Some(Key::Q) },
    DrumLane { name: "Md Cga", pitch: MID_CONGA, color: Color32::from_rgb(160, 100, 50), key: Some(Key::W) },
    DrumLane { name: "Lo Cga", pitch: LOW_CONGA, color: Color32::from_rgb(140, 80, 40), key: Some(Key::E) },
    DrumLane { name: "Maraca", pitch: MARACAS, color: Color32::from_rgb(100, 180, 100), key: Some(Key::R) },
    DrumLane { name: "Claves", pitch: CLAVES, color: Color32::from_rgb(160, 140, 120), key: Some(Key::T) },
];

/// Actions returned from drum roll
#[derive(Clone, Debug)]
pub enum DrumRollAction {
    None,
    TogglePlayback {
        clip_start_sample: u64,
    },
    ClipModified,
    PlayNote { pitch: u8, velocity: u8 },
    SetLoopRegion {
        start_sample: u64,
        end_sample: u64,
    },
}

/// Drum roll step sequencer panel
pub struct DrumRollPanel {
    /// Pixels per beat horizontally
    pub pixels_per_beat: f32,
    /// Height of each drum lane
    lane_height: f32,
    /// Width of the label column
    label_width: f32,
    /// Horizontal scroll offset in beats
    scroll_x: f64,
    /// Currently editing clip
    editing_clip: Option<ClipId>,
    /// Grid subdivision (0.25 = 16th notes)
    grid_subdivision: f64,
    /// Snap to grid enabled
    pub snap_to_grid: bool,
    /// Loop selection state (start beat when Ctrl+dragging)
    loop_drag_start: Option<f64>,
    /// Keys currently pressed (for drum triggering)
    keys_pressed: std::collections::HashSet<Key>,
}

impl Default for DrumRollPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DrumRollPanel {
    pub fn new() -> Self {
        Self {
            pixels_per_beat: 80.0,
            lane_height: 24.0, // Smaller for 16 lanes
            label_width: 60.0,
            scroll_x: 0.0,
            editing_clip: None,
            grid_subdivision: 0.25, // 16th notes
            snap_to_grid: true,
            loop_drag_start: None,
            keys_pressed: std::collections::HashSet::new(),
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        clip: &mut MidiClip,
        bpm: f64,
        sample_rate: u32,
        clip_start_sample: u64,
        playback_position: u64,
        _clipboard: &DawClipboard,
    ) -> DrumRollAction {
        let mut action = DrumRollAction::None;

        // Reset editing state if clip changed
        if self.editing_clip != Some(clip.id) {
            self.editing_clip = Some(clip.id);
            self.scroll_x = 0.0;
        }

        let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
        let clip_length_beats = clip.length_samples as f64 / samples_per_beat;

        // Toolbar
        ui.horizontal(|ui| {
            if ui.button("▶").clicked() {
                action = DrumRollAction::TogglePlayback {
                    clip_start_sample,
                };
            }
            ui.separator();
            ui.label("Grid:");
            if ui.selectable_label(self.grid_subdivision == 0.25, "1/16").clicked() {
                self.grid_subdivision = 0.25;
            }
            if ui.selectable_label(self.grid_subdivision == 0.5, "1/8").clicked() {
                self.grid_subdivision = 0.5;
            }
            if ui.selectable_label(self.grid_subdivision == 1.0, "1/4").clicked() {
                self.grid_subdivision = 1.0;
            }
            ui.separator();
            ui.checkbox(&mut self.snap_to_grid, "Snap");
        });

        ui.separator();

        // Main grid area
        let available = ui.available_size();
        let grid_width = available.x - self.label_width;
        let grid_height = self.lane_height * DRUM_LANES.len() as f32;

        let (response, painter) = ui.allocate_painter(
            Vec2::new(available.x, grid_height.max(available.y)),
            Sense::click_and_drag(),
        );

        let rect = response.rect;
        let grid_rect = Rect::from_min_size(
            rect.min + Vec2::new(self.label_width, 0.0),
            Vec2::new(grid_width, grid_height),
        );

        // Handle scroll
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
            if scroll_delta.x.abs() > 0.1 {
                self.scroll_x = (self.scroll_x - scroll_delta.x as f64 / self.pixels_per_beat as f64)
                    .max(0.0)
                    .min(clip_length_beats - 4.0);
            }
        }

        // Background
        painter.rect_filled(rect, 0.0, Color32::from_gray(30));
        painter.rect_filled(grid_rect, 0.0, Color32::from_gray(40));

        // Draw lane labels and backgrounds
        for (i, lane) in DRUM_LANES.iter().enumerate() {
            let y = rect.min.y + i as f32 * self.lane_height;
            let label_rect = Rect::from_min_size(
                Pos2::new(rect.min.x, y),
                Vec2::new(self.label_width, self.lane_height),
            );

            // Alternate lane backgrounds
            let bg_color = if i % 2 == 0 {
                Color32::from_gray(35)
            } else {
                Color32::from_gray(45)
            };
            painter.rect_filled(
                Rect::from_min_size(grid_rect.min + Vec2::new(0.0, i as f32 * self.lane_height), Vec2::new(grid_width, self.lane_height)),
                0.0,
                bg_color,
            );

            // Label background
            painter.rect_filled(label_rect, 0.0, lane.color.gamma_multiply(0.3));

            // Label text
            painter.text(
                label_rect.center(),
                egui::Align2::CENTER_CENTER,
                lane.name,
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );

            // Lane separator
            painter.hline(
                rect.min.x..=rect.max.x,
                y + self.lane_height,
                Stroke::new(1.0, Color32::from_gray(60)),
            );
        }

        // Draw grid lines
        let visible_beats = grid_width as f64 / self.pixels_per_beat as f64;
        let start_beat = self.scroll_x;
        let end_beat = start_beat + visible_beats;

        // Beat lines
        let mut beat = (start_beat / self.grid_subdivision).floor() * self.grid_subdivision;
        while beat <= end_beat && beat <= clip_length_beats {
            let x = grid_rect.min.x + ((beat - start_beat) * self.pixels_per_beat as f64) as f32;
            let is_bar = (beat % 4.0).abs() < 0.001;
            let is_beat = (beat % 1.0).abs() < 0.001;

            let color = if is_bar {
                Color32::from_gray(100)
            } else if is_beat {
                Color32::from_gray(70)
            } else {
                Color32::from_gray(50)
            };

            painter.vline(x, grid_rect.min.y..=grid_rect.max.y, Stroke::new(1.0, color));
            beat += self.grid_subdivision;
        }

        // Draw existing notes
        for note in &clip.notes {
            let Some(lane_idx) = DRUM_LANES.iter().position(|l| l.pitch == note.pitch) else {
                continue;
            };

            let note_beat = note.start_tick as f64 / clip.ppq as f64;
            if note_beat < start_beat - 1.0 || note_beat > end_beat + 1.0 {
                continue;
            }

            let x = grid_rect.min.x + ((note_beat - start_beat) * self.pixels_per_beat as f64) as f32;
            let y = grid_rect.min.y + lane_idx as f32 * self.lane_height;

            // Note cell (step)
            let cell_width = (self.grid_subdivision * self.pixels_per_beat as f64) as f32 * 0.8;
            let cell_height = self.lane_height * 0.7;
            let cell_rect = Rect::from_center_size(
                Pos2::new(x + cell_width / 2.0, y + self.lane_height / 2.0),
                Vec2::new(cell_width, cell_height),
            );

            // Velocity affects opacity
            let alpha = 0.5 + (note.velocity as f32 / 127.0) * 0.5;
            let color = DRUM_LANES[lane_idx].color.gamma_multiply(alpha);
            painter.rect_filled(cell_rect, 4.0, color);
            painter.rect_stroke(cell_rect, 4.0, Stroke::new(1.0, Color32::WHITE.gamma_multiply(0.5)), StrokeKind::Inside);
        }

        // Draw playhead
        let playback_beat = if playback_position >= clip_start_sample {
            (playback_position - clip_start_sample) as f64 / samples_per_beat
        } else {
            0.0
        };

        if playback_beat >= start_beat && playback_beat <= end_beat {
            let x = grid_rect.min.x + ((playback_beat - start_beat) * self.pixels_per_beat as f64) as f32;
            painter.vline(x, grid_rect.min.y..=grid_rect.max.y, Stroke::new(2.0, Color32::from_rgb(255, 100, 100)));
        }

        // Handle Ctrl+drag for loop selection
        let ctrl_held = ui.input(|i| i.modifiers.ctrl);
        if ctrl_held && response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                if grid_rect.contains(pos) {
                    let rel_x = pos.x - grid_rect.min.x;
                    let beat = start_beat + rel_x as f64 / self.pixels_per_beat as f64;
                    let snapped = (beat / self.grid_subdivision).floor() * self.grid_subdivision;
                    self.loop_drag_start = Some(snapped);
                }
            }
        }

        if ctrl_held && response.dragged() && self.loop_drag_start.is_some() {
            // Draw loop selection preview
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(start_beat_loop) = self.loop_drag_start {
                    let rel_x = pos.x - grid_rect.min.x;
                    let current_beat = start_beat + rel_x as f64 / self.pixels_per_beat as f64;
                    let snapped_current = (current_beat / self.grid_subdivision).round() * self.grid_subdivision;

                    let (loop_start, loop_end) = if start_beat_loop < snapped_current {
                        (start_beat_loop, snapped_current)
                    } else {
                        (snapped_current, start_beat_loop)
                    };

                    // Draw selection rectangle
                    let x1 = grid_rect.min.x + ((loop_start - start_beat) * self.pixels_per_beat as f64) as f32;
                    let x2 = grid_rect.min.x + ((loop_end - start_beat) * self.pixels_per_beat as f64) as f32;
                    let sel_rect = Rect::from_x_y_ranges(x1..=x2, grid_rect.min.y..=grid_rect.max.y);
                    painter.rect_filled(sel_rect, 0.0, Color32::from_rgba_unmultiplied(100, 150, 255, 50));
                    painter.rect_stroke(sel_rect, 0.0, Stroke::new(2.0, Color32::from_rgb(100, 150, 255)), StrokeKind::Inside);
                }
            }
        }

        if response.drag_stopped() && self.loop_drag_start.is_some() {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(start_beat_loop) = self.loop_drag_start.take() {
                    let rel_x = pos.x - grid_rect.min.x;
                    let current_beat = start_beat + rel_x as f64 / self.pixels_per_beat as f64;
                    let snapped_current = (current_beat / self.grid_subdivision).round() * self.grid_subdivision;

                    let (loop_start, loop_end) = if start_beat_loop < snapped_current {
                        (start_beat_loop, snapped_current)
                    } else {
                        (snapped_current, start_beat_loop)
                    };

                    if (loop_end - loop_start).abs() > 0.1 {
                        let start_sample = clip_start_sample + (loop_start * samples_per_beat) as u64;
                        let end_sample = clip_start_sample + (loop_end * samples_per_beat) as u64;
                        action = DrumRollAction::SetLoopRegion { start_sample, end_sample };
                    }
                }
            }
        }

        // Handle keyboard input for drum triggering
        for lane in &DRUM_LANES {
            if let Some(key) = lane.key {
                let pressed = ui.input(|i| i.key_pressed(key));
                let released = ui.input(|i| i.key_released(key));

                if pressed && !self.keys_pressed.contains(&key) {
                    self.keys_pressed.insert(key);
                    action = DrumRollAction::PlayNote { pitch: lane.pitch, velocity: 100 };
                }
                if released {
                    self.keys_pressed.remove(&key);
                }
            }
        }

        // Handle click to toggle notes (only when not Ctrl+dragging)
        if !ctrl_held && response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if grid_rect.contains(pos) {
                    let rel_x = pos.x - grid_rect.min.x;
                    let rel_y = pos.y - grid_rect.min.y;

                    let beat = start_beat + rel_x as f64 / self.pixels_per_beat as f64;
                    let lane_idx = (rel_y / self.lane_height) as usize;

                    if lane_idx < DRUM_LANES.len() {
                        let snapped_beat = if self.snap_to_grid {
                            (beat / self.grid_subdivision).floor() * self.grid_subdivision
                        } else {
                            beat
                        };

                        let tick = (snapped_beat * clip.ppq as f64) as u64;
                        let pitch = DRUM_LANES[lane_idx].pitch;

                        // Check if note exists at this position
                        let existing_idx = clip.notes.iter().position(|n| {
                            n.pitch == pitch && (n.start_tick as i64 - tick as i64).abs() < (clip.ppq / 4) as i64
                        });

                        if let Some(idx) = existing_idx {
                            // Remove existing note
                            clip.notes.remove(idx);
                            action = DrumRollAction::ClipModified;
                        } else {
                            // Add new note (1/16th duration for drums)
                            let duration = (self.grid_subdivision * clip.ppq as f64) as u64;
                            let note = MidiNote::new(pitch, 100, tick, duration);
                            clip.add_note(note);

                            // Preview the sound
                            action = DrumRollAction::PlayNote { pitch, velocity: 100 };
                        }
                    }
                }
            }
        }

        action
    }
}
