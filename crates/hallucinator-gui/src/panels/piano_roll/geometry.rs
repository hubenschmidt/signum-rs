use egui::{Pos2, Rect};
use hallucinator_core::MidiClip;

use super::types::DragMode;
use super::PianoRollPanel;

impl PianoRollPanel {
    /// Map keyboard key to MIDI pitch with octave offset
    /// Bottom row: Z=C, S=C#, X=D, D=D#, C=E, V=F, G=F#, B=G, H=G#, N=A, J=A#, M=B
    /// Top row: Q=C+1oct, 2=C#+1oct, W=D+1oct, etc.
    /// Base octave 0 = C3 (MIDI 48) for bottom row, C4 (60) for top row
    pub(super) fn key_to_pitch(&self, key: egui::Key) -> Option<u8> {
        let base_offset = self.keyboard_octave as i16 * 12;

        let base_pitch: i16 = match key {
            // Bottom row - base C3 octave (MIDI 48-59)
            egui::Key::Z => 48,
            egui::Key::S => 49,
            egui::Key::X => 50,
            egui::Key::D => 51,
            egui::Key::C => 52,
            egui::Key::V => 53,
            egui::Key::G => 54,
            egui::Key::B => 55,
            egui::Key::H => 56,
            egui::Key::N => 57,
            egui::Key::J => 58,
            egui::Key::M => 59,
            // Top row - base C4 octave (MIDI 60-71)
            egui::Key::Q => 60,
            egui::Key::Num2 => 61,
            egui::Key::W => 62,
            egui::Key::Num3 => 63,
            egui::Key::E => 64,
            egui::Key::R => 65,
            egui::Key::Num5 => 66,
            egui::Key::T => 67,
            egui::Key::Num6 => 68,
            egui::Key::Y => 69,
            egui::Key::Num7 => 70,
            egui::Key::U => 71,
            egui::Key::I => 72,
            egui::Key::Num9 => 73,
            egui::Key::O => 74,
            egui::Key::Num0 => 75,
            egui::Key::P => 76,
            _ => return None,
        };

        let pitch = base_pitch + base_offset;
        if (0..=127).contains(&pitch) {
            Some(pitch as u8)
        } else {
            None
        }
    }

    /// Get list of piano keys to check
    pub(super) fn piano_keys() -> &'static [egui::Key] {
        &[
            egui::Key::Z, egui::Key::S, egui::Key::X, egui::Key::D, egui::Key::C,
            egui::Key::V, egui::Key::G, egui::Key::B, egui::Key::H, egui::Key::N,
            egui::Key::J, egui::Key::M, egui::Key::Q, egui::Key::Num2, egui::Key::W,
            egui::Key::Num3, egui::Key::E, egui::Key::R, egui::Key::Num5, egui::Key::T,
            egui::Key::Num6, egui::Key::Y, egui::Key::Num7, egui::Key::U, egui::Key::I,
            egui::Key::Num9, egui::Key::O, egui::Key::Num0, egui::Key::P,
        ]
    }

    pub(super) fn pitch_to_y(&self, pitch: u8, rect: Rect) -> f32 {
        let relative_pitch = (pitch.saturating_sub(self.visible_pitch_min)) as f32;
        let inverted = self.visible_pitches as f32 - 1.0 - relative_pitch;
        rect.top() + inverted * self.key_height
    }

    pub(super) fn pos_to_beat_pitch(&self, pos: Pos2, rect: Rect) -> (f64, u8) {
        let beat = self.scroll_x + (pos.x - rect.left()) as f64 / self.pixels_per_beat as f64;
        let row = ((pos.y - rect.top()) / self.key_height).floor();
        let inverted_pitch = ((self.visible_pitches - 1) as f32 - row).max(0.0) as u8;
        let pitch = self.visible_pitch_min.saturating_add(inverted_pitch).min(127);
        (beat, pitch)
    }

    pub(super) fn find_note_at(
        &self,
        clip: &MidiClip,
        beat: f64,
        pitch: u8,
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

    /// Find note and determine if we're clicking to move or resize
    pub(super) fn find_note_drag_target(&self, clip: &MidiClip, beat: f64, pitch: u8, grid_rect: Rect) -> Option<(usize, DragMode)> {
        let resize_threshold = 8.0;

        for (idx, note) in clip.notes.iter().enumerate() {
            if note.pitch != pitch {
                continue;
            }
            let note_start = note.start_tick as f64 / clip.ppq as f64;
            let note_end = note_start + note.duration_ticks as f64 / clip.ppq as f64;

            if beat >= note_start && beat < note_end {
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
}
