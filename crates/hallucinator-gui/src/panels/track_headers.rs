//! Track headers panel - track controls column (left of arrange view)

use egui::{Color32, Rect, Sense, Stroke, Ui, Vec2};
use hallucinator_core::Track;

/// Action returned from track headers
pub enum TrackHeaderAction {
    None,
    SelectTrack(usize),
    ToggleMute(usize),
    ToggleSolo(usize),
    ToggleArm(usize),
    SetVolume(usize, f32),
    SetPan(usize, f32),
    DeleteTrack(usize),
    AddAudioTrack,
    AddMidiTrack,
    RenameTrack(usize, String),
}

/// Track headers panel state
pub struct TrackHeadersPanel {
    pub track_height: f32,
    pub ruler_height: f32,
}

impl TrackHeadersPanel {
    pub fn new() -> Self {
        Self {
            track_height: 80.0,
            ruler_height: 24.0, // Match arrange panel ruler
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        tracks: &[Track],
        selected_track_idx: Option<usize>,
    ) -> TrackHeaderAction {
        let mut action = TrackHeaderAction::None;

        // Force vertical layout
        ui.vertical(|ui| {
            // Add spacer to match arrange panel ruler height
            ui.add_space(self.ruler_height);

            // Add each track header
            for (idx, track) in tracks.iter().enumerate() {
                let header_action = self.draw_track_header(
                    ui,
                    idx,
                    track,
                    selected_track_idx == Some(idx),
                );

                if !matches!(header_action, TrackHeaderAction::None) {
                    action = header_action;
                }
            }

            // Fill remaining space and handle right-click for adding tracks
            let remaining = ui.available_size();
            if remaining.y > 0.0 {
                let (rect, response) = ui.allocate_exact_size(remaining, Sense::click());

                // Draw empty area background
                ui.painter().rect_filled(rect, 0.0, Color32::from_gray(35));

                // Right-click on empty area to add track
                if response.secondary_clicked() {
                    action = TrackHeaderAction::AddAudioTrack; // Default action
                }

                response.context_menu(|ui| {
                    if ui.button("Add Audio Track").clicked() {
                        action = TrackHeaderAction::AddAudioTrack;
                        ui.close_menu();
                    }
                    if ui.button("Add MIDI Track").clicked() {
                        action = TrackHeaderAction::AddMidiTrack;
                        ui.close_menu();
                    }
                });
            }
        });

        action
    }

    fn draw_track_header(
        &mut self,
        ui: &mut Ui,
        idx: usize,
        track: &Track,
        is_selected: bool,
    ) -> TrackHeaderAction {
        let mut action = TrackHeaderAction::None;

        let width = ui.available_width();
        let header_size = Vec2::new(width, self.track_height);

        let (rect, response) = ui.allocate_exact_size(header_size, Sense::click());
        let painter = ui.painter();

        // Background
        let bg_color = if is_selected {
            Color32::from_rgb(50, 60, 80)
        } else {
            Color32::from_gray(45)
        };
        painter.rect_filled(rect, 0.0, bg_color);
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, Color32::from_gray(30)),
            egui::StrokeKind::Inside,
        );

        // Click to select
        if response.clicked() {
            action = TrackHeaderAction::SelectTrack(idx);
        }

        // Right-click context menu
        response.context_menu(|ui| {
            if ui.button("Delete Track").clicked() {
                action = TrackHeaderAction::DeleteTrack(idx);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Add Audio Track").clicked() {
                action = TrackHeaderAction::AddAudioTrack;
                ui.close_menu();
            }
            if ui.button("Add MIDI Track").clicked() {
                action = TrackHeaderAction::AddMidiTrack;
                ui.close_menu();
            }
        });

        // Track name (top)
        let name_y = rect.top() + 4.0;
        painter.text(
            egui::pos2(rect.left() + 4.0, name_y),
            egui::Align2::LEFT_TOP,
            &track.name,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );

        // Control buttons row
        let btn_y = rect.top() + 22.0;
        let btn_size = 16.0;
        let btn_spacing = 2.0;
        let mut btn_x = rect.left() + 4.0;

        // M (Mute) button
        let mute_rect = Rect::from_min_size(egui::pos2(btn_x, btn_y), Vec2::splat(btn_size));
        let mute_color = if track.mute {
            Color32::from_rgb(200, 150, 50)
        } else {
            Color32::from_gray(70)
        };
        painter.rect_filled(mute_rect, 2.0, mute_color);
        painter.text(
            mute_rect.center(),
            egui::Align2::CENTER_CENTER,
            "M",
            egui::FontId::proportional(9.0),
            Color32::WHITE,
        );
        let mute_response = ui.interact(mute_rect, ui.id().with(("mute", idx)), Sense::click());
        if mute_response.clicked() {
            action = TrackHeaderAction::ToggleMute(idx);
        }
        btn_x += btn_size + btn_spacing;

        // S (Solo) button
        let solo_rect = Rect::from_min_size(egui::pos2(btn_x, btn_y), Vec2::splat(btn_size));
        let solo_color = if track.solo {
            Color32::from_rgb(200, 180, 50)
        } else {
            Color32::from_gray(70)
        };
        painter.rect_filled(solo_rect, 2.0, solo_color);
        painter.text(
            solo_rect.center(),
            egui::Align2::CENTER_CENTER,
            "S",
            egui::FontId::proportional(9.0),
            Color32::WHITE,
        );
        let solo_response = ui.interact(solo_rect, ui.id().with(("solo", idx)), Sense::click());
        if solo_response.clicked() {
            action = TrackHeaderAction::ToggleSolo(idx);
        }
        btn_x += btn_size + btn_spacing;

        // Arm button
        let arm_rect = Rect::from_min_size(egui::pos2(btn_x, btn_y), Vec2::splat(btn_size));
        let arm_color = if track.armed {
            Color32::from_rgb(200, 60, 60)
        } else {
            Color32::from_gray(70)
        };
        painter.rect_filled(arm_rect, 2.0, arm_color);
        painter.text(
            arm_rect.center(),
            egui::Align2::CENTER_CENTER,
            "●",
            egui::FontId::proportional(8.0),
            Color32::WHITE,
        );
        let arm_response = ui.interact(arm_rect, ui.id().with(("arm", idx)), Sense::click());
        if arm_response.clicked() {
            action = TrackHeaderAction::ToggleArm(idx);
        }

        // Volume slider (horizontal at bottom)
        let vol_y = rect.top() + 44.0;
        let vol_width = width - 8.0;
        let vol_rect = Rect::from_min_size(
            egui::pos2(rect.left() + 4.0, vol_y),
            Vec2::new(vol_width, 10.0),
        );
        painter.rect_filled(vol_rect, 2.0, Color32::from_gray(30));

        let vol_fill_width = (track.volume.clamp(0.0, 1.5) / 1.5) * vol_width;
        let vol_fill = Rect::from_min_size(vol_rect.min, Vec2::new(vol_fill_width, 10.0));
        painter.rect_filled(vol_fill, 2.0, Color32::from_rgb(80, 140, 80));

        let vol_response = ui.interact(vol_rect, ui.id().with(("vol", idx)), Sense::drag());
        if vol_response.dragged() {
            if let Some(pos) = vol_response.interact_pointer_pos() {
                let vol = ((pos.x - vol_rect.left()) / vol_width) * 1.5;
                action = TrackHeaderAction::SetVolume(idx, vol.clamp(0.0, 1.5));
            }
        }

        // Pan slider (horizontal below volume)
        let pan_y = rect.top() + 58.0;
        let pan_rect = Rect::from_min_size(
            egui::pos2(rect.left() + 4.0, pan_y),
            Vec2::new(vol_width, 8.0),
        );
        painter.rect_filled(pan_rect, 2.0, Color32::from_gray(30));

        let pan_center = pan_rect.center().x;
        let pan_pos = pan_center + (track.pan * (vol_width / 2.0 - 4.0));
        painter.rect_filled(
            Rect::from_center_size(egui::pos2(pan_pos, pan_rect.center().y), Vec2::new(6.0, 6.0)),
            2.0,
            Color32::from_rgb(120, 120, 180),
        );

        let pan_response = ui.interact(pan_rect, ui.id().with(("pan", idx)), Sense::drag());
        if pan_response.dragged() {
            if let Some(pos) = pan_response.interact_pointer_pos() {
                let pan = ((pos.x - pan_center) / (vol_width / 2.0)).clamp(-1.0, 1.0);
                action = TrackHeaderAction::SetPan(idx, pan);
            }
        }

        action
    }
}

impl Default for TrackHeadersPanel {
    fn default() -> Self {
        Self::new()
    }
}
