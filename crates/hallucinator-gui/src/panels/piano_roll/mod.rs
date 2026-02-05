//! Piano roll panel for MIDI editing

mod drawing;
mod geometry;
mod input;
mod types;

pub use types::PianoRollAction;
use types::{LoopDragMode, LoopSelection, NoteDragState};

use std::collections::HashSet;

use crate::clipboard::DawClipboard;
use egui::{Color32, Pos2, Rect, Sense, Ui, Vec2};
use hallucinator_core::MidiClip;

/// Piano roll editor panel
pub struct PianoRollPanel {
    /// Pixels per beat horizontally
    pub pixels_per_beat: f32,
    /// Height of each piano key row
    key_height: f32,
    /// Horizontal scroll offset in beats
    scroll_x: f64,
    /// Currently selected notes (clip_id, note_index)
    selected_notes: HashSet<usize>,
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
    /// Loop drag state (for moving/resizing existing loop)
    loop_drag: Option<(LoopDragMode, f64)>, // (mode, original_beat)
    /// Loop selection drag state (for creating new loop via drag)
    loop_select_drag: Option<f64>, // start beat of drag
    /// Currently pressed keyboard keys (for note preview)
    pressed_keys: HashSet<egui::Key>,
    /// Keyboard octave offset (0 = C3/C4 base, +1 = C4/C5, -1 = C2/C3)
    keyboard_octave: i8,
    /// Currently active MIDI pitches (for visual feedback on piano keys)
    active_pitches: HashSet<u8>,
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
            selected_notes: HashSet::new(),
            note_drag: None,
            visible_pitch_min: 36, // C2
            visible_pitches: 48,   // 4 octaves
            snap_to_grid: true,
            grid_subdivision: 0.25, // 16th notes default
            loop_selection: None,
            loop_drag: None,
            loop_select_drag: None,
            pressed_keys: HashSet::new(),
            keyboard_octave: 0,
            active_pitches: HashSet::new(),
        }
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
        _clipboard: &DawClipboard,
    ) -> PianoRollAction {
        let mut action = PianoRollAction::None;
        let mut modified = false;

        // Calculate grid subdivision based on zoom level
        self.grid_subdivision = if self.pixels_per_beat >= 160.0 {
            0.125
        } else if self.pixels_per_beat >= 80.0 {
            0.25
        } else if self.pixels_per_beat >= 40.0 {
            0.5
        } else {
            1.0
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

            if ui.button("Oct-").clicked() {
                self.keyboard_octave = (self.keyboard_octave - 1).max(-2);
                self.active_pitches.clear();
            }
            let octave_name = match self.keyboard_octave {
                -2 => "C1-C3", -1 => "C2-C4", 0 => "C3-C5",
                1 => "C4-C6", 2 => "C5-C7", _ => "C3-C5",
            };
            ui.label(format!("Oct: {}", octave_name));
            if ui.button("Oct+").clicked() {
                self.keyboard_octave = (self.keyboard_octave + 1).min(2);
                self.active_pitches.clear();
            }
            ui.separator();

            if ui.button("Delete Selected").clicked() {
                if let Some(a) = self.delete_selected_notes(clip) {
                    action = a;
                    modified = true;
                }
            }

            if let Some(ref sel) = self.loop_selection {
                ui.separator();
                ui.label(format!("Loop: {:.1}-{:.1} bars", sel.start_beat / 4.0, sel.end_beat / 4.0));
                if ui.button("Clear Loop").clicked() {
                    self.loop_selection = None;
                }
            }
        });

        ui.separator();

        // Layout
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

        let piano_roll_id = ui.id().with("piano_roll_focus");
        let (response, painter) = ui.allocate_painter(available.size(), Sense::click_and_drag());

        if response.clicked() || response.drag_started() || response.hovered() {
            ui.memory_mut(|mem| mem.request_focus(piano_roll_id));
        }
        let has_focus = ui.memory(|mem| mem.has_focus(piano_roll_id));

        // Background
        painter.rect_filled(grid_rect, 0.0, Color32::from_gray(25));
        painter.rect_filled(piano_rect, 0.0, Color32::from_gray(40));

        // Drawing
        let beats_visible = grid_rect.width() as f64 / self.pixels_per_beat as f64;
        self.draw_piano_keys(&painter, piano_rect);
        self.draw_grid(&painter, grid_rect, beats_visible);
        self.draw_notes(&painter, grid_rect, clip);

        let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
        self.draw_playhead(&painter, grid_rect, clip_start_sample, clip.length_samples, playback_position, samples_per_beat);

        // Input handling
        if response.drag_started() {
            let ctrl_held = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
            self.handle_drag_start(&response, grid_rect, clip, ctrl_held);
        }
        if response.dragged() {
            modified |= self.handle_drag_continue(&response, grid_rect, clip);
        }

        let drag_end_action = self.handle_drag_end(&response, clip_start_sample, samples_per_beat);
        if !matches!(drag_end_action, PianoRollAction::None) {
            action = drag_end_action;
        }

        let (click_modified, click_action) = self.handle_click(&response, grid_rect, clip);
        if click_modified {
            modified = true;
            if !matches!(click_action, PianoRollAction::None) {
                action = click_action;
            }
        }

        let (del_modified, del_action) = self.handle_delete_key(ui, clip);
        if del_modified {
            modified = true;
            action = del_action;
        }

        if has_focus || response.hovered() {
            let kb_action = self.handle_keyboard_piano(ui);
            if !matches!(kb_action, PianoRollAction::None) {
                action = kb_action;
            }
        }

        // Loop handling
        self.handle_loop_drag(&response, grid_rect);
        self.handle_loop_right_click(&response, grid_rect);

        let ctx_action = self.handle_loop_context_menu(&response, clip_start_sample, samples_per_beat);
        if !matches!(ctx_action, PianoRollAction::None) {
            action = ctx_action;
        }

        self.draw_loop_selection(&painter, grid_rect);
        self.handle_scroll_zoom(ui, &response);

        if modified {
            return PianoRollAction::ClipModified;
        }
        action
    }

}
