//! Device rack panel - horizontal signal chain for selected track

use egui::{Color32, Rect, ScrollArea, Sense, Stroke, Ui, Vec2};

/// Info about a device in the chain
#[derive(Clone)]
pub struct DeviceInfo {
    pub id: u64,
    pub name: String,
    pub is_instrument: bool,
    pub is_bypassed: bool,
    pub has_ui: bool,
}

/// Action returned from device rack
#[derive(Clone)]
pub enum DeviceRackAction {
    None,
    OpenPluginWindow(u64),
    ToggleBypass(u64),
    RemoveDevice(u64),
    AddEffect,
}

/// Device rack panel state
pub struct DeviceRackPanel {
    selected_device_id: Option<u64>,
}

impl DeviceRackPanel {
    pub fn new() -> Self {
        Self {
            selected_device_id: None,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        track_name: Option<&str>,
        instrument: Option<DeviceInfo>,
        effects: &[DeviceInfo],
    ) -> DeviceRackAction {
        let mut action = DeviceRackAction::None;

        // Header
        ui.horizontal(|ui| {
            ui.heading("Device Rack");
            if let Some(name) = track_name {
                ui.separator();
                ui.label(name);
            }
        });

        ui.separator();

        if track_name.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("Select a track to view devices");
            });
            return action;
        }

        ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                // Draw instrument slot (if MIDI track)
                if let Some(inst) = &instrument {
                    let device_action = self.draw_device(ui, inst, true);
                    if !matches!(device_action, DeviceRackAction::None) {
                        action = device_action;
                    }

                    // Arrow connector
                    ui.label("→");
                }

                // Draw effect chain
                for (idx, effect) in effects.iter().enumerate() {
                    let device_action = self.draw_device(ui, effect, false);
                    if !matches!(device_action, DeviceRackAction::None) {
                        action = device_action;
                    }

                    if idx < effects.len() - 1 {
                        ui.label("→");
                    }
                }

                // Add effect button
                ui.add_space(8.0);
                let add_btn = ui.button("+ Add Effect");
                if add_btn.clicked() {
                    action = DeviceRackAction::AddEffect;
                }
            });
        });

        action
    }

    fn draw_device(&mut self, ui: &mut Ui, device: &DeviceInfo, is_instrument: bool) -> DeviceRackAction {
        let mut action = DeviceRackAction::None;

        let device_width = 100.0;
        let device_height = 80.0;

        let (response, painter) = ui.allocate_painter(
            Vec2::new(device_width, device_height),
            Sense::click(),
        );
        let rect = response.rect;

        let is_selected = self.selected_device_id == Some(device.id);

        // Background
        let bg_color = if device.is_bypassed {
            Color32::from_gray(40)
        } else if is_instrument {
            Color32::from_rgb(60, 80, 100)
        } else {
            Color32::from_rgb(70, 70, 90)
        };

        painter.rect_filled(rect, 4.0, bg_color);

        let border_color = if is_selected {
            Color32::from_rgb(150, 180, 220)
        } else {
            Color32::from_gray(80)
        };
        painter.rect_stroke(rect, 4.0, Stroke::new(1.5, border_color), egui::StrokeKind::Outside);

        // Device type icon
        let icon = if is_instrument { "🎹" } else { "🎛" };
        painter.text(
            egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
            egui::Align2::LEFT_TOP,
            icon,
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );

        // Has UI indicator
        if device.has_ui {
            painter.text(
                egui::pos2(rect.right() - 6.0, rect.top() + 4.0),
                egui::Align2::RIGHT_TOP,
                "🔲",
                egui::FontId::proportional(10.0),
                Color32::from_gray(150),
            );
        }

        // Device name (truncate if needed)
        let name_display = if device.name.len() > 12 {
            format!("{}...", &device.name[..10])
        } else {
            device.name.clone()
        };

        painter.text(
            egui::pos2(rect.center().x, rect.top() + 24.0),
            egui::Align2::CENTER_TOP,
            &name_display,
            egui::FontId::proportional(10.0),
            if device.is_bypassed {
                Color32::from_gray(120)
            } else {
                Color32::WHITE
            },
        );

        // Bypass indicator
        if device.is_bypassed {
            painter.text(
                egui::pos2(rect.center().x, rect.center().y + 8.0),
                egui::Align2::CENTER_CENTER,
                "BYPASSED",
                egui::FontId::proportional(8.0),
                Color32::from_rgb(200, 150, 50),
            );
        }

        // Bypass button at bottom-left
        let bypass_rect = Rect::from_min_size(
            egui::pos2(rect.left() + 4.0, rect.bottom() - 20.0),
            Vec2::new(16.0, 16.0),
        );
        let bypass_color = if device.is_bypassed {
            Color32::from_rgb(200, 150, 50)
        } else {
            Color32::from_gray(60)
        };
        painter.rect_filled(bypass_rect, 2.0, bypass_color);
        painter.text(
            bypass_rect.center(),
            egui::Align2::CENTER_CENTER,
            "B",
            egui::FontId::proportional(9.0),
            Color32::WHITE,
        );

        let bypass_response = ui.allocate_rect(bypass_rect, Sense::click());
        if bypass_response.clicked() {
            action = DeviceRackAction::ToggleBypass(device.id);
        }

        // Remove button at bottom-right (only for effects)
        if !is_instrument {
            let remove_rect = Rect::from_min_size(
                egui::pos2(rect.right() - 20.0, rect.bottom() - 20.0),
                Vec2::new(16.0, 16.0),
            );
            painter.rect_filled(remove_rect, 2.0, Color32::from_gray(60));
            painter.text(
                remove_rect.center(),
                egui::Align2::CENTER_CENTER,
                "×",
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );

            let remove_response = ui.allocate_rect(remove_rect, Sense::click());
            if remove_response.clicked() {
                action = DeviceRackAction::RemoveDevice(device.id);
            }
        }

        // Handle main click/double-click
        if response.clicked() {
            self.selected_device_id = Some(device.id);
        }
        if response.double_clicked() {
            action = DeviceRackAction::OpenPluginWindow(device.id);
        }

        action
    }
}

impl Default for DeviceRackPanel {
    fn default() -> Self {
        Self::new()
    }
}
