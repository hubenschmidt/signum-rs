//! Piano roll panel for MIDI editing

use std::collections::HashSet;

use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use signum_core::{ClipId, MidiClip, MidiNote};

/// Actions returned from piano roll
#[derive(Clone, Debug)]
pub enum PianoRollAction {
    None,
    /// Toggle playback within the clip boundaries
    TogglePlayback {
        clip_start_sample: u64,
        clip_end_sample: u64,
    },
    ClipModified,
    /// Set loop region from selection
    SetLoopRegion {
        start_sample: u64,
        end_sample: u64,
    },
    /// Play a preview note (when not playing)
    PlayNote {
        pitch: u8,
        velocity: u8,
    },
    /// Stop a preview note
    StopNote {
        pitch: u8,
    },
    /// Record a note during playback
    RecordNote {
        pitch: u8,
        velocity: u8,
    },
}

/// State for dragging a note
#[derive(Clone, Copy)]
enum DragMode {
    Move,       // Moving the entire note
    ResizeEnd,  // Resizing from the right edge
}

/// State for drawing a new note
struct DrawState {
    start_beat: f64,
    pitch: u8,
}

/// Note drag state
struct NoteDragState {
    note_idx: usize,
    mode: DragMode,
    original_start_tick: u64,
    original_duration_ticks: u64,
    original_pitch: u8,
    drag_start_beat: f64,
    drag_start_pitch: u8,
}

/// Loop selection state
#[derive(Clone)]
struct LoopSelection {
    start_beat: f64,
    end_beat: f64,
}

/// What part of the loop is being dragged
#[derive(Clone, Copy)]
enum LoopDragMode {
    Start,  // Dragging left edge
    End,    // Dragging right edge
    Move,   // Dragging entire selection
}

/// Piano roll editor panel
pub struct PianoRollPanel {
    /// Pixels per beat horizontally
    pub pixels_per_beat: f32,
    /// Height of each piano key row
    key_height: f32,
    /// Horizontal scroll offset in beats
    scroll_x: f64,
    /// Vertical scroll offset (pitch)
    scroll_y: i32,
    /// Currently selected notes (clip_id, note_index)
    selected_notes: HashSet<usize>,
    /// Currently editing clip
    editing_clip: Option<ClipId>,
    /// Note drawing state
    draw_state: Option<DrawState>,
    /// Note drag state
    note_drag: Option<NoteDragState>,
    /// Lowest visible pitch
    visible_pitch_min: u8,
    /// Number of visible pitches
    visible_pitches: u8,
    /// Snap to grid enabled
    pub snap_to_grid: bool,
    /// Grid subdivision (1.0 = quarter, 0.5 = 8th, 0.25 = 16th, etc.)
    grid_subdivision: f64,
    /// Loop selection (beat range)
    loop_selection: Option<LoopSelection>,
    /// Loop drag state
    loop_drag: Option<(LoopDragMode, f64)>, // (mode, original_beat)
    /// Currently pressed keyboard keys (for note preview)
    pressed_keys: HashSet<egui::Key>,
    /// Keyboard octave offset (0 = C3/C4 base, +1 = C4/C5, -1 = C2/C3)
    keyboard_octave: i8,
}

impl Default for PianoRollPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl PianoRollPanel {
    pub fn new() -> Self {
        Self {
            pixels_per_beat: 60.0,
            key_height: 16.0,
            scroll_x: 0.0,
            scroll_y: 0,
            selected_notes: HashSet::new(),
            editing_clip: None,
            draw_state: None,
            note_drag: None,
            visible_pitch_min: 36, // C2
            visible_pitches: 48,   // 4 octaves
            snap_to_grid: true,
            grid_subdivision: 0.25, // 16th notes default
            loop_selection: None,
            loop_drag: None,
            pressed_keys: HashSet::new(),
            keyboard_octave: 0,
        }
    }

    /// Map keyboard key to MIDI pitch with octave offset
    /// Bottom row: Z=C, S=C#, X=D, D=D#, C=E, V=F, G=F#, B=G, H=G#, N=A, J=A#, M=B
    /// Top row: Q=C+1oct, 2=C#+1oct, W=D+1oct, etc.
    /// Base octave 0 = C3 (MIDI 48) for bottom row, C4 (60) for top row
    fn key_to_pitch(&self, key: egui::Key) -> Option<u8> {
        let base_offset = self.keyboard_octave as i16 * 12;

        let base_pitch: i16 = match key {
            // Bottom row - base C3 octave (MIDI 48-59)
            egui::Key::Z => 48, // C
            egui::Key::S => 49, // C#
            egui::Key::X => 50, // D
            egui::Key::D => 51, // D#
            egui::Key::C => 52, // E
            egui::Key::V => 53, // F
            egui::Key::G => 54, // F#
            egui::Key::B => 55, // G
            egui::Key::H => 56, // G#
            egui::Key::N => 57, // A
            egui::Key::J => 58, // A#
            egui::Key::M => 59, // B
            // Top row - base C4 octave (MIDI 60-71)
            egui::Key::Q => 60, // C (middle C)
            egui::Key::Num2 => 61, // C#
            egui::Key::W => 62, // D
            egui::Key::Num3 => 63, // D#
            egui::Key::E => 64, // E
            egui::Key::R => 65, // F
            egui::Key::Num5 => 66, // F#
            egui::Key::T => 67, // G
            egui::Key::Num6 => 68, // G#
            egui::Key::Y => 69, // A
            egui::Key::Num7 => 70, // A#
            egui::Key::U => 71, // B
            egui::Key::I => 72, // C5
            egui::Key::Num9 => 73, // C#5
            egui::Key::O => 74, // D5
            egui::Key::Num0 => 75, // D#5
            egui::Key::P => 76, // E5
            _ => return None,
        };

        let pitch = base_pitch + base_offset;
        if pitch >= 0 && pitch <= 127 {
            Some(pitch as u8)
        } else {
            None
        }
    }

    /// Get list of piano keys to check
    fn piano_keys() -> &'static [egui::Key] {
        &[
            egui::Key::Z, egui::Key::S, egui::Key::X, egui::Key::D, egui::Key::C,
            egui::Key::V, egui::Key::G, egui::Key::B, egui::Key::H, egui::Key::N,
            egui::Key::J, egui::Key::M, egui::Key::Q, egui::Key::Num2, egui::Key::W,
            egui::Key::Num3, egui::Key::E, egui::Key::R, egui::Key::Num5, egui::Key::T,
            egui::Key::Num6, egui::Key::Y, egui::Key::Num7, egui::Key::U, egui::Key::I,
            egui::Key::Num9, egui::Key::O, egui::Key::Num0, egui::Key::P,
        ]
    }

    /// Set the clip being edited
    pub fn set_clip(&mut self, clip_id: ClipId) {
        self.editing_clip = Some(clip_id);
        self.selected_notes.clear();
    }

    /// Clear the current clip
    pub fn clear_clip(&mut self) {
        self.editing_clip = None;
        self.selected_notes.clear();
    }

    /// Get the currently editing clip ID
    pub fn editing_clip(&self) -> Option<ClipId> {
        self.editing_clip
    }

    /// Render the piano roll UI
    /// Returns action to be handled by app
    /// - clip_start_sample: where the clip starts in the timeline
    /// - playback_position: current playback position in samples
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        clip: &mut MidiClip,
        bpm: f64,
        sample_rate: u32,
        clip_start_sample: u64,
        playback_position: u64,
    ) -> PianoRollAction {
        let mut action = PianoRollAction::None;
        let mut modified = false;

        // Calculate grid subdivision based on zoom level
        self.grid_subdivision = if self.pixels_per_beat >= 160.0 {
            0.125 // 32nd notes
        } else if self.pixels_per_beat >= 80.0 {
            0.25  // 16th notes
        } else if self.pixels_per_beat >= 40.0 {
            0.5   // 8th notes
        } else {
            1.0   // quarter notes
        };

        // Toolbar
        ui.horizontal(|ui| {
            ui.label("Piano Roll");
            ui.separator();

            if ui.button("-").clicked() {
                self.pixels_per_beat = (self.pixels_per_beat * 0.8).max(20.0);
            }
            if ui.button("+").clicked() {
                self.pixels_per_beat = (self.pixels_per_beat * 1.25).min(200.0);
            }
            ui.label(format!("{:.0} px/beat", self.pixels_per_beat));

            ui.separator();

            ui.checkbox(&mut self.snap_to_grid, "Snap");

            ui.separator();

            // Octave shift for keyboard input
            if ui.button("Oct-").clicked() {
                self.keyboard_octave = (self.keyboard_octave - 1).max(-2);
            }
            let octave_name = match self.keyboard_octave {
                -2 => "C1-C3",
                -1 => "C2-C4",
                0 => "C3-C5",
                1 => "C4-C6",
                2 => "C5-C7",
                _ => "C3-C5",
            };
            ui.label(format!("Oct: {}", octave_name));
            if ui.button("Oct+").clicked() {
                self.keyboard_octave = (self.keyboard_octave + 1).min(2);
            }

            ui.separator();

            if ui.button("Delete Selected").clicked() && !self.selected_notes.is_empty() {
                let mut indices: Vec<_> = self.selected_notes.iter().copied().collect();
                indices.sort_by(|a, b| b.cmp(a)); // Sort descending to remove from end first
                for idx in indices {
                    clip.remove_note(idx);
                }
                self.selected_notes.clear();
                modified = true;
            }

            // Loop selection info
            if let Some(ref sel) = self.loop_selection {
                ui.separator();
                ui.label(format!("Loop: {:.1}-{:.1} bars", sel.start_beat / 4.0, sel.end_beat / 4.0));
                if ui.button("Clear Loop").clicked() {
                    self.loop_selection = None;
                }
            }
        });

        ui.separator();

        // Main piano roll area
        let available = ui.available_rect_before_wrap();
        let piano_width = 40.0;
        let grid_rect = Rect::from_min_size(
            Pos2::new(available.left() + piano_width, available.top()),
            Vec2::new(available.width() - piano_width, available.height()),
        );
        let piano_rect = Rect::from_min_size(
            available.min,
            Vec2::new(piano_width, available.height()),
        );

        // Use a focusable Area for keyboard input
        let piano_roll_id = ui.id().with("piano_roll_focus");
        let (response, painter) = ui.allocate_painter(available.size(), Sense::click_and_drag());

        // Request focus on click so we receive keyboard events
        if response.clicked() || response.drag_started() {
            ui.memory_mut(|mem| mem.request_focus(piano_roll_id));
        }

        // Also request focus when hovered and keys are pressed
        if response.hovered() {
            ui.memory_mut(|mem| mem.request_focus(piano_roll_id));
        }

        let has_focus = ui.memory(|mem| mem.has_focus(piano_roll_id));

        // Background
        painter.rect_filled(grid_rect, 0.0, Color32::from_gray(25));
        painter.rect_filled(piano_rect, 0.0, Color32::from_gray(40));

        // Calculate visible range
        let beats_visible = grid_rect.width() as f64 / self.pixels_per_beat as f64;
        let _clip_length_beats = self.samples_to_beats(clip.length_samples, clip.ppq, bpm, sample_rate);

        // Draw piano keys
        self.draw_piano_keys(&painter, piano_rect);

        // Draw grid lines
        self.draw_grid(&painter, grid_rect, beats_visible);

        // Draw notes
        self.draw_notes(&painter, grid_rect, clip, bpm, sample_rate);

        // Draw playhead cursor
        let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
        let clip_end_sample = clip_start_sample + clip.length_samples;

        // Only draw if playhead is within clip range
        if playback_position >= clip_start_sample && playback_position <= clip_end_sample {
            let position_in_clip = playback_position - clip_start_sample;
            let position_beat = position_in_clip as f64 / samples_per_beat;
            let playhead_x = grid_rect.left() + ((position_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;

            if playhead_x >= grid_rect.left() && playhead_x <= grid_rect.right() {
                // Draw playhead line
                painter.line_segment(
                    [Pos2::new(playhead_x, grid_rect.top()), Pos2::new(playhead_x, grid_rect.bottom())],
                    Stroke::new(2.0, Color32::from_rgb(255, 100, 100)),
                );

                // Draw playhead triangle at top
                let triangle = vec![
                    Pos2::new(playhead_x, grid_rect.top()),
                    Pos2::new(playhead_x - 6.0, grid_rect.top() - 8.0),
                    Pos2::new(playhead_x + 6.0, grid_rect.top() - 8.0),
                ];
                painter.add(egui::Shape::convex_polygon(
                    triangle,
                    Color32::from_rgb(255, 100, 100),
                    Stroke::NONE,
                ));
            }
        }

        // Handle note dragging
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                if grid_rect.contains(pos) {
                    let (beat, pitch) = self.pos_to_beat_pitch(pos, grid_rect);

                    // Check if starting drag on a note
                    if let Some((note_idx, drag_mode)) = self.find_note_drag_target(clip, beat, pitch, grid_rect) {
                        let note = &clip.notes[note_idx];
                        self.note_drag = Some(NoteDragState {
                            note_idx,
                            mode: drag_mode,
                            original_start_tick: note.start_tick,
                            original_duration_ticks: note.duration_ticks,
                            original_pitch: note.pitch,
                            drag_start_beat: beat,
                            drag_start_pitch: pitch,
                        });
                        self.selected_notes.clear();
                        self.selected_notes.insert(note_idx);
                    }
                }
            }
        }

        if response.dragged() {
            if let Some(ref drag_state) = self.note_drag {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (beat, pitch) = self.pos_to_beat_pitch(pos, grid_rect);
                    let beat_delta = beat - drag_state.drag_start_beat;
                    let pitch_delta = pitch as i32 - drag_state.drag_start_pitch as i32;

                    if let Some(note) = clip.notes.get_mut(drag_state.note_idx) {
                        match drag_state.mode {
                            DragMode::Move => {
                                // Move note position and pitch
                                let new_start_beat = (drag_state.original_start_tick as f64 / clip.ppq as f64) + beat_delta;
                                let snapped_beat = if self.snap_to_grid {
                                    (new_start_beat / self.grid_subdivision).round() * self.grid_subdivision
                                } else {
                                    new_start_beat
                                };
                                note.start_tick = ((snapped_beat.max(0.0)) * clip.ppq as f64) as u64;
                                note.pitch = (drag_state.original_pitch as i32 + pitch_delta).clamp(0, 127) as u8;
                            }
                            DragMode::ResizeEnd => {
                                // Resize note duration
                                let new_duration_beats = (drag_state.original_duration_ticks as f64 / clip.ppq as f64) + beat_delta;
                                let snapped_duration = if self.snap_to_grid {
                                    (new_duration_beats / self.grid_subdivision).round() * self.grid_subdivision
                                } else {
                                    new_duration_beats
                                };
                                let min_duration = self.grid_subdivision;
                                note.duration_ticks = ((snapped_duration.max(min_duration)) * clip.ppq as f64) as u64;
                            }
                        }
                        modified = true;
                    }
                }
            }
        }

        if response.drag_stopped() {
            self.note_drag = None;
        }

        // Handle click (only if not dragging)
        if response.clicked() && self.note_drag.is_none() {
            if let Some(pos) = response.interact_pointer_pos() {
                if grid_rect.contains(pos) {
                    let (beat, pitch) = self.pos_to_beat_pitch(pos, grid_rect);

                    // Check if clicking on existing note
                    let clicked_note = self.find_note_at(clip, beat, pitch, bpm, sample_rate);

                    if let Some(note_idx) = clicked_note {
                        // Select/deselect note
                        if ui.input(|i| i.modifiers.shift) {
                            if self.selected_notes.contains(&note_idx) {
                                self.selected_notes.remove(&note_idx);
                            } else {
                                self.selected_notes.insert(note_idx);
                            }
                        } else {
                            self.selected_notes.clear();
                            self.selected_notes.insert(note_idx);
                        }
                    } else {
                        // Create new note snapped to grid
                        let quantized_beat = if self.snap_to_grid {
                            (beat / self.grid_subdivision).floor() * self.grid_subdivision
                        } else {
                            beat
                        };
                        let start_tick = (quantized_beat * clip.ppq as f64) as u64;
                        let duration_ticks = (self.grid_subdivision * clip.ppq as f64) as u64;

                        let note = MidiNote::new(pitch, 100, start_tick, duration_ticks);
                        clip.add_note(note);
                        self.selected_notes.clear();
                        modified = true;
                    }
                }
            }
        }

        // Handle spacebar for playback toggle (within clip boundaries)
        if response.hovered() {
            if ui.input(|i| i.key_pressed(egui::Key::Space)) {
                action = PianoRollAction::TogglePlayback {
                    clip_start_sample,
                    clip_end_sample: clip_start_sample + clip.length_samples,
                };
            }
        }

        // Handle delete key
        if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            if !self.selected_notes.is_empty() {
                let mut indices: Vec<_> = self.selected_notes.iter().copied().collect();
                indices.sort_by(|a, b| b.cmp(a));
                for idx in indices {
                    clip.remove_note(idx);
                }
                self.selected_notes.clear();
                modified = true;
            }
        }

        // Handle keyboard piano input when piano roll has focus or is hovered
        if has_focus || response.hovered() {
            // Collect note events to handle multiple keys
            let mut note_on: Option<u8> = None;
            let mut note_off: Option<u8> = None;

            // Check octave shift keys
            if ui.input(|i| i.key_pressed(egui::Key::Minus)) {
                self.keyboard_octave = (self.keyboard_octave - 1).max(-2);
            }
            if ui.input(|i| i.key_pressed(egui::Key::Equals)) {
                self.keyboard_octave = (self.keyboard_octave + 1).min(2);
            }

            // Check piano keys - use key_pressed for more reliable detection
            for &key in Self::piano_keys() {
                let is_pressed = ui.input(|i| i.key_down(key));
                let was_pressed = self.pressed_keys.contains(&key);

                if is_pressed && !was_pressed {
                    // Key just pressed
                    self.pressed_keys.insert(key);
                    if let Some(pitch) = self.key_to_pitch(key) {
                        note_on = Some(pitch);
                    }
                } else if !is_pressed && was_pressed {
                    // Key just released
                    self.pressed_keys.remove(&key);
                    if let Some(pitch) = self.key_to_pitch(key) {
                        note_off = Some(pitch);
                    }
                }
            }

            // Return note actions (prioritize note on over note off)
            // Only set if action hasn't been set by other handlers
            if let Some(pitch) = note_on {
                action = PianoRollAction::PlayNote { pitch, velocity: 100 };
            } else if let Some(pitch) = note_off {
                action = PianoRollAction::StopNote { pitch };
            }
        }

        // Handle loop selection dragging
        let samples_per_beat = sample_rate as f64 * 60.0 / bpm;

        // Check if mouse is over loop selection edges for dragging
        if let Some(ref selection) = self.loop_selection.clone() {
            let start_x = grid_rect.left() + ((selection.start_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;
            let end_x = grid_rect.left() + ((selection.end_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;
            let handle_width = 8.0;

            if response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    // Check if clicking on loop handles (higher priority than notes)
                    if pos.y >= grid_rect.top() - 20.0 && pos.y <= grid_rect.top() + 20.0 {
                        if (pos.x - start_x).abs() < handle_width {
                            self.loop_drag = Some((LoopDragMode::Start, selection.start_beat));
                        } else if (pos.x - end_x).abs() < handle_width {
                            self.loop_drag = Some((LoopDragMode::End, selection.end_beat));
                        } else if pos.x > start_x && pos.x < end_x {
                            self.loop_drag = Some((LoopDragMode::Move, selection.start_beat));
                        }
                    }
                }
            }
        }

        if response.dragged() {
            if let Some((mode, original_beat)) = self.loop_drag {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (beat, _) = self.pos_to_beat_pitch(pos, grid_rect);
                    // Snap to beats
                    let snapped_beat = (beat / 1.0).round() * 1.0;

                    if let Some(ref mut selection) = self.loop_selection {
                        match mode {
                            LoopDragMode::Start => {
                                selection.start_beat = snapped_beat.max(0.0).min(selection.end_beat - 1.0);
                            }
                            LoopDragMode::End => {
                                selection.end_beat = snapped_beat.max(selection.start_beat + 1.0);
                            }
                            LoopDragMode::Move => {
                                let delta = snapped_beat - original_beat;
                                let duration = selection.end_beat - selection.start_beat;
                                let new_start = (selection.start_beat + delta).max(0.0);
                                selection.start_beat = new_start;
                                selection.end_beat = new_start + duration;
                            }
                        }
                    }
                }
            }
        }

        if response.drag_stopped() && self.loop_drag.is_some() {
            self.loop_drag = None;
        }

        // Handle right-click for loop selection
        if response.secondary_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if grid_rect.contains(pos) {
                    let (beat, _) = self.pos_to_beat_pitch(pos, grid_rect);
                    // Snap to bar (4 beats)
                    let bar = (beat / 4.0).floor() * 4.0;

                    if self.loop_selection.is_none() {
                        // Start a new selection at clicked bar
                        self.loop_selection = Some(LoopSelection {
                            start_beat: bar,
                            end_beat: bar + 4.0, // Default to 1 bar
                        });
                    }
                }
            }
        }

        // Context menu for loop selection
        if self.loop_selection.is_some() {
            response.context_menu(|ui| {
                if ui.button("Set Loop Region").clicked() {
                    if let Some(ref selection) = self.loop_selection {
                        let start_sample = clip_start_sample + (selection.start_beat * samples_per_beat) as u64;
                        let end_sample = clip_start_sample + (selection.end_beat * samples_per_beat) as u64;
                        action = PianoRollAction::SetLoopRegion { start_sample, end_sample };
                    }
                    ui.close_menu();
                }
                if ui.button("Clear Selection").clicked() {
                    self.loop_selection = None;
                    ui.close_menu();
                }
            });
        }

        // Draw loop selection with draggable handles
        if let Some(ref selection) = self.loop_selection {
            let start_x = grid_rect.left() + ((selection.start_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;
            let end_x = grid_rect.left() + ((selection.end_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;

            if end_x > grid_rect.left() && start_x < grid_rect.right() {
                // Selection highlight
                let sel_rect = Rect::from_min_max(
                    Pos2::new(start_x.max(grid_rect.left()), grid_rect.top()),
                    Pos2::new(end_x.min(grid_rect.right()), grid_rect.bottom()),
                );
                painter.rect_filled(sel_rect, 0.0, Color32::from_rgba_unmultiplied(100, 150, 255, 30));
                painter.rect_stroke(sel_rect, 0.0, Stroke::new(2.0, Color32::from_rgb(100, 150, 255)), StrokeKind::Inside);

                // Draw drag handles (triangles at top)
                let handle_color = Color32::from_rgb(80, 130, 220);

                // Start handle (left triangle)
                if start_x >= grid_rect.left() {
                    let triangle = vec![
                        Pos2::new(start_x, grid_rect.top()),
                        Pos2::new(start_x - 8.0, grid_rect.top() - 12.0),
                        Pos2::new(start_x + 8.0, grid_rect.top() - 12.0),
                    ];
                    painter.add(egui::Shape::convex_polygon(triangle, handle_color, Stroke::NONE));
                    // Vertical line
                    painter.line_segment(
                        [Pos2::new(start_x, grid_rect.top()), Pos2::new(start_x, grid_rect.bottom())],
                        Stroke::new(2.0, handle_color),
                    );
                }

                // End handle (right triangle)
                if end_x <= grid_rect.right() {
                    let triangle = vec![
                        Pos2::new(end_x, grid_rect.top()),
                        Pos2::new(end_x - 8.0, grid_rect.top() - 12.0),
                        Pos2::new(end_x + 8.0, grid_rect.top() - 12.0),
                    ];
                    painter.add(egui::Shape::convex_polygon(triangle, handle_color, Stroke::NONE));
                    // Vertical line
                    painter.line_segment(
                        [Pos2::new(end_x, grid_rect.top()), Pos2::new(end_x, grid_rect.bottom())],
                        Stroke::new(2.0, handle_color),
                    );
                }
            }
        }

        // Handle scroll and zoom
        if response.hovered() {
            let (scroll_delta, modifiers) = ui.input(|i| {
                (i.smooth_scroll_delta + i.raw_scroll_delta, i.modifiers)
            });

            if modifiers.ctrl || modifiers.command {
                // Ctrl/Cmd + scroll for zoom
                if scroll_delta.y.abs() > 0.1 {
                    let zoom_factor = 1.0 + scroll_delta.y * 0.008;
                    self.pixels_per_beat = (self.pixels_per_beat * zoom_factor).clamp(20.0, 200.0);
                }
            } else {
                // Regular scroll for panning
                if scroll_delta.x.abs() > 0.0 || scroll_delta.y.abs() > 0.0 {
                    self.scroll_x = (self.scroll_x - scroll_delta.x as f64 / self.pixels_per_beat as f64).max(0.0);
                    // Vertical scroll moves view up/down pitches
                    let pitch_scroll = (scroll_delta.y / self.key_height) as i32;
                    self.visible_pitch_min = (self.visible_pitch_min as i32 + pitch_scroll)
                        .clamp(0, 127 - self.visible_pitches as i32) as u8;
                }
            }
        }

        if modified {
            return PianoRollAction::ClipModified;
        }
        action
    }

    /// Find note and determine if we're clicking to move or resize
    fn find_note_drag_target(&self, clip: &MidiClip, beat: f64, pitch: u8, grid_rect: Rect) -> Option<(usize, DragMode)> {
        let resize_threshold = 8.0; // pixels from right edge to trigger resize

        for (idx, note) in clip.notes.iter().enumerate() {
            if note.pitch != pitch {
                continue;
            }
            let note_start = note.start_tick as f64 / clip.ppq as f64;
            let note_end = note_start + note.duration_ticks as f64 / clip.ppq as f64;

            if beat >= note_start && beat < note_end {
                // Check if we're near the right edge (resize) or in the body (move)
                let note_end_x = grid_rect.left() + ((note_end - self.scroll_x) * self.pixels_per_beat as f64) as f32;
                let click_x = grid_rect.left() + ((beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;

                if note_end_x - click_x < resize_threshold {
                    return Some((idx, DragMode::ResizeEnd));
                }
                return Some((idx, DragMode::Move));
            }
        }
        None
    }

    fn draw_piano_keys(&self, painter: &egui::Painter, rect: Rect) {
        let pitch_max = (self.visible_pitch_min + self.visible_pitches).min(127);

        for pitch in self.visible_pitch_min..pitch_max {
            let y = self.pitch_to_y(pitch, rect);
            let key_rect = Rect::from_min_size(
                Pos2::new(rect.left(), y),
                Vec2::new(rect.width(), self.key_height),
            );

            let is_black = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
            let color = if is_black {
                Color32::from_gray(30)
            } else {
                Color32::from_gray(60)
            };

            painter.rect_filled(key_rect, 0.0, color);
            painter.rect_stroke(key_rect, 0.0, Stroke::new(0.5, Color32::from_gray(20)), StrokeKind::Inside);

            // Label C notes
            if pitch % 12 == 0 {
                let octave = pitch / 12 - 1;
                painter.text(
                    Pos2::new(rect.left() + 2.0, y + 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("C{}", octave),
                    egui::FontId::proportional(9.0),
                    Color32::WHITE,
                );
            }
        }
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: Rect, beats_visible: f64) {
        let start_beat = self.scroll_x;
        let end_beat = start_beat + beats_visible;

        // Draw vertical lines with dynamic subdivision based on zoom
        let grid_step = self.grid_subdivision;
        let pixels_per_grid = self.pixels_per_beat as f64 * grid_step;

        // Only draw if grid lines have minimum spacing
        if pixels_per_grid >= 8.0 {
            let mut pos = (start_beat / grid_step).floor() * grid_step;
            while pos <= end_beat {
                let x = rect.left() + ((pos - start_beat) * self.pixels_per_beat as f64) as f32;

                // Determine line type: bar > beat > subdivision
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

    fn draw_notes(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        clip: &MidiClip,
        bpm: f64,
        sample_rate: u32,
    ) {
        for (idx, note) in clip.notes.iter().enumerate() {
            let start_beat = note.start_tick as f64 / clip.ppq as f64;
            let duration_beats = note.duration_ticks as f64 / clip.ppq as f64;

            // Skip if outside visible range
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

            // Clip to visible area
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

    fn pitch_to_y(&self, pitch: u8, rect: Rect) -> f32 {
        let relative_pitch = (pitch.saturating_sub(self.visible_pitch_min)) as f32;
        let inverted = (self.visible_pitches as f32 - 1.0 - relative_pitch); // Higher pitches at top
        rect.top() + inverted * self.key_height
    }

    fn pos_to_beat_pitch(&self, pos: Pos2, rect: Rect) -> (f64, u8) {
        let beat = self.scroll_x + (pos.x - rect.left()) as f64 / self.pixels_per_beat as f64;
        let relative_y = (pos.y - rect.top()) / self.key_height;
        let inverted_pitch = (self.visible_pitches as f32 - 1.0 - relative_y) as u8;
        let pitch = self.visible_pitch_min.saturating_add(inverted_pitch).min(127);
        (beat, pitch)
    }

    fn find_note_at(
        &self,
        clip: &MidiClip,
        beat: f64,
        pitch: u8,
        _bpm: f64,
        _sample_rate: u32,
    ) -> Option<usize> {
        for (idx, note) in clip.notes.iter().enumerate() {
            if note.pitch != pitch {
                continue;
            }
            let note_start = note.start_tick as f64 / clip.ppq as f64;
            let note_end = note_start + note.duration_ticks as f64 / clip.ppq as f64;
            if beat >= note_start && beat < note_end {
                return Some(idx);
            }
        }
        None
    }

    fn samples_to_beats(&self, samples: u64, _ppq: u16, bpm: f64, sample_rate: u32) -> f64 {
        let seconds = samples as f64 / sample_rate as f64;
        seconds * bpm / 60.0
    }
}
