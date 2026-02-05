use egui::{Pos2, Rect, Ui};
use hallucinator_core::{MidiClip, MidiNote};

use super::types::{DragMode, LoopDragMode, LoopSelection, NoteDragState, PianoRollAction};
use super::PianoRollPanel;

impl PianoRollPanel {
    /// Delete all selected notes, returning their pitches for note-off messages.
    pub(super) fn delete_selected_notes(&mut self, clip: &mut MidiClip) -> Option<PianoRollAction> {
        if self.selected_notes.is_empty() {
            return None;
        }

        let mut indices: Vec<_> = self.selected_notes.iter().copied().collect();
        indices.sort_by(|a, b| b.cmp(a));

        let pitches: Vec<u8> = indices.iter()
            .filter_map(|&idx| clip.notes.get(idx).map(|n| n.pitch))
            .collect();

        for idx in indices {
            clip.remove_note(idx);
        }
        self.selected_notes.clear();

        if pitches.is_empty() {
            return None;
        }
        Some(PianoRollAction::StopNotes { pitches })
    }

    /// Handle click on grid — toggle note (remove existing or create new).
    /// Returns (modified, action).
    pub(super) fn handle_click(
        &mut self,
        response: &egui::Response,
        grid_rect: Rect,
        clip: &mut MidiClip,
    ) -> (bool, PianoRollAction) {
        if !response.clicked() || self.note_drag.is_some() {
            return (false, PianoRollAction::None);
        }

        let Some(pos) = response.interact_pointer_pos() else {
            return (false, PianoRollAction::None);
        };
        if !grid_rect.contains(pos) {
            return (false, PianoRollAction::None);
        }

        let (beat, pitch) = self.pos_to_beat_pitch(pos, grid_rect);
        let clicked_note = self.find_note_at(clip, beat, pitch);

        if let Some(note_idx) = clicked_note {
            let pitch = clip.notes.get(note_idx).map(|n| n.pitch);
            clip.remove_note(note_idx);
            self.selected_notes.remove(&note_idx);
            let action = pitch
                .map(|p| PianoRollAction::StopNotes { pitches: vec![p] })
                .unwrap_or(PianoRollAction::None);
            return (true, action);
        }

        // Create new note snapped to grid
        let quantized_beat = if self.snap_to_grid {
            (beat / self.grid_subdivision).floor() * self.grid_subdivision
        } else {
            beat
        };
        let start_tick = (quantized_beat * clip.ppq as f64) as u64;
        let duration_ticks = (self.grid_subdivision * clip.ppq as f64) as u64;

        clip.add_note(MidiNote::new(pitch, 100, start_tick, duration_ticks));
        self.selected_notes.clear();
        (true, PianoRollAction::None)
    }

    /// Handle delete/backspace key.
    pub(super) fn handle_delete_key(&mut self, ui: &Ui, clip: &mut MidiClip) -> (bool, PianoRollAction) {
        let delete_pressed = ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if !delete_pressed {
            return (false, PianoRollAction::None);
        }

        match self.delete_selected_notes(clip) {
            Some(action) => (true, action),
            None => (false, PianoRollAction::None),
        }
    }

    /// Handle keyboard piano input. Returns action for note on/off.
    pub(super) fn handle_keyboard_piano(&mut self, ui: &Ui) -> PianoRollAction {
        // Octave shift keys
        if ui.input(|i| i.key_pressed(egui::Key::Minus)) {
            self.keyboard_octave = (self.keyboard_octave - 1).max(-2);
            self.active_pitches.clear();
        }
        if ui.input(|i| i.key_pressed(egui::Key::Equals)) {
            self.keyboard_octave = (self.keyboard_octave + 1).min(2);
            self.active_pitches.clear();
        }

        let mut note_on: Option<u8> = None;
        let mut note_off: Option<u8> = None;

        for &key in Self::piano_keys() {
            let is_pressed = ui.input(|i| i.key_down(key));
            let was_pressed = self.pressed_keys.contains(&key);

            if is_pressed && !was_pressed {
                self.pressed_keys.insert(key);
                if let Some(pitch) = self.key_to_pitch(key) {
                    self.active_pitches.insert(pitch);
                    note_on = Some(pitch);
                }
            } else if !is_pressed && was_pressed {
                self.pressed_keys.remove(&key);
                if let Some(pitch) = self.key_to_pitch(key) {
                    self.active_pitches.remove(&pitch);
                    note_off = Some(pitch);
                }
            }
        }

        if let Some(pitch) = note_on {
            return PianoRollAction::PlayNote { pitch, velocity: 100 };
        }
        if let Some(pitch) = note_off {
            return PianoRollAction::StopNote { pitch };
        }
        PianoRollAction::None
    }

    /// Handle scroll and zoom via mouse wheel.
    pub(super) fn handle_scroll_zoom(&mut self, ui: &Ui, response: &egui::Response) {
        if !response.hovered() {
            return;
        }

        let (scroll_delta, modifiers) = ui.input(|i| {
            (i.smooth_scroll_delta + i.raw_scroll_delta, i.modifiers)
        });

        if modifiers.ctrl || modifiers.command {
            if scroll_delta.y.abs() > 0.1 {
                let zoom_factor = 1.0 + scroll_delta.y * 0.008;
                self.pixels_per_beat = (self.pixels_per_beat * zoom_factor).clamp(20.0, 200.0);
            }
            return;
        }

        if scroll_delta.x.abs() > 0.0 || scroll_delta.y.abs() > 0.0 {
            self.scroll_x = (self.scroll_x - scroll_delta.x as f64 / self.pixels_per_beat as f64).max(0.0);
            let pitch_scroll = (scroll_delta.y / self.key_height) as i32;
            self.visible_pitch_min = (self.visible_pitch_min as i32 + pitch_scroll)
                .clamp(0, 127 - self.visible_pitches as i32) as u8;
        }
    }

    /// Handle drag end — emit loop region if selection was made.
    pub(super) fn handle_drag_end(
        &mut self,
        response: &egui::Response,
        clip_start_sample: u64,
        samples_per_beat: f64,
    ) -> PianoRollAction {
        if !response.drag_stopped() {
            return PianoRollAction::None;
        }

        let mut action = PianoRollAction::None;

        if self.loop_select_drag.is_some() {
            if let Some(ref sel) = self.loop_selection {
                let start_sample = clip_start_sample + (sel.start_beat * samples_per_beat) as u64;
                let end_sample = clip_start_sample + (sel.end_beat * samples_per_beat) as u64;
                action = PianoRollAction::SetLoopRegion { start_sample, end_sample };
            }
        }

        self.note_drag = None;
        self.loop_select_drag = None;
        action
    }

    /// Handle loop handle dragging (start/end/move).
    pub(super) fn handle_loop_drag(&mut self, response: &egui::Response, grid_rect: Rect) {
        // Check drag start on loop handles
        if response.drag_started() {
            if let Some(ref selection) = self.loop_selection.clone() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let start_x = grid_rect.left() + ((selection.start_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;
                    let end_x = grid_rect.left() + ((selection.end_beat - self.scroll_x) * self.pixels_per_beat as f64) as f32;
                    let handle_width = 8.0;

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

        // Continue drag
        if response.dragged() {
            if let Some((mode, original_beat)) = self.loop_drag {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (beat, _) = self.pos_to_beat_pitch(pos, grid_rect);
                    let snapped_beat = beat.round();

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
    }

    /// Handle right-click to create new loop selection.
    pub(super) fn handle_loop_right_click(&mut self, response: &egui::Response, grid_rect: Rect) {
        if !response.secondary_clicked() {
            return;
        }
        let Some(pos) = response.interact_pointer_pos() else { return };
        if !grid_rect.contains(pos) {
            return;
        }
        if self.loop_selection.is_some() {
            return;
        }

        let (beat, _) = self.pos_to_beat_pitch(pos, grid_rect);
        let bar = (beat / 4.0).floor() * 4.0;
        self.loop_selection = Some(LoopSelection {
            start_beat: bar,
            end_beat: bar + 4.0,
        });
    }

    /// Show context menu for loop selection. Returns action if "Set Loop Region" clicked.
    pub(super) fn handle_loop_context_menu(
        &mut self,
        response: &egui::Response,
        clip_start_sample: u64,
        samples_per_beat: f64,
    ) -> PianoRollAction {
        if self.loop_selection.is_none() {
            return PianoRollAction::None;
        }

        let mut action = PianoRollAction::None;
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
        action
    }

    /// Handle drag start — either note drag or loop selection.
    pub(super) fn handle_drag_start(&mut self, response: &egui::Response, grid_rect: Rect, clip: &MidiClip, ctrl_held: bool) {
        let Some(pos) = response.interact_pointer_pos() else { return };
        if !grid_rect.contains(pos) { return };

        let (beat, pitch) = self.pos_to_beat_pitch(pos, grid_rect);

        if ctrl_held {
            let snapped_beat = (beat / 0.25).floor() * 0.25;
            self.loop_select_drag = Some(snapped_beat);
            self.loop_selection = Some(LoopSelection {
                start_beat: snapped_beat,
                end_beat: snapped_beat + 0.25,
            });
            return;
        }

        let Some((note_idx, drag_mode)) = self.find_note_drag_target(clip, beat, pitch, grid_rect) else { return };

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

    /// Handle drag continue — update note position or loop selection.
    pub(super) fn handle_drag_continue(&mut self, response: &egui::Response, grid_rect: Rect, clip: &mut MidiClip) -> bool {
        let Some(pos) = response.interact_pointer_pos() else { return false };

        if let Some(ref drag_state) = self.note_drag.clone() {
            return self.update_note_drag(pos, grid_rect, clip, &drag_state);
        }

        let Some(start_beat) = self.loop_select_drag else { return false };
        let (beat, _) = self.pos_to_beat_pitch(pos, grid_rect);
        let snapped_beat = (beat / 0.25).floor() * 0.25;

        let (sel_start, sel_end) = if snapped_beat < start_beat {
            (snapped_beat, start_beat)
        } else {
            (start_beat, snapped_beat.max(start_beat + 0.25))
        };

        self.loop_selection = Some(LoopSelection {
            start_beat: sel_start,
            end_beat: sel_end,
        });
        false
    }

    /// Update note position/size during drag.
    fn update_note_drag(&mut self, pos: Pos2, grid_rect: Rect, clip: &mut MidiClip, drag_state: &NoteDragState) -> bool {
        let (beat, pitch) = self.pos_to_beat_pitch(pos, grid_rect);
        let beat_delta = beat - drag_state.drag_start_beat;
        let pitch_delta = pitch as i32 - drag_state.drag_start_pitch as i32;

        let Some(note) = clip.notes.get_mut(drag_state.note_idx) else { return false };

        let snap_factor = if self.snap_to_grid { self.grid_subdivision } else { 0.001 };

        match drag_state.mode {
            DragMode::Move => {
                let new_start_beat = (drag_state.original_start_tick as f64 / clip.ppq as f64) + beat_delta;
                let snapped_beat = (new_start_beat / snap_factor).round() * snap_factor;
                note.start_tick = (snapped_beat.max(0.0) * clip.ppq as f64) as u64;
                note.pitch = (drag_state.original_pitch as i32 + pitch_delta).clamp(0, 127) as u8;
            }
            DragMode::ResizeEnd => {
                let new_duration_beats = (drag_state.original_duration_ticks as f64 / clip.ppq as f64) + beat_delta;
                let snapped_duration = (new_duration_beats / snap_factor).round() * snap_factor;
                let min_duration = self.grid_subdivision;
                note.duration_ticks = (snapped_duration.max(min_duration) * clip.ppq as f64) as u64;
            }
        }
        true
    }
}
