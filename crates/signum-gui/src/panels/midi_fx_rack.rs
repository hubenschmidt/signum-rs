//! MIDI FX rack panel - chain of MIDI effects per track (Hapax-style)

use egui::{Color32, ComboBox, Rect, Sense, Slider, Stroke, Ui, Vec2};
use signum_core::{MidiEffect, MidiFxChain, MidiFxParam};

/// Action returned from MIDI FX rack
#[derive(Clone)]
pub enum MidiFxRackAction {
    None,
    AddEffect(MidiEffectType),
    RemoveEffect(usize),
    ToggleBypass(usize),
    MoveEffect { from: usize, to: usize },
    SetParam { effect_idx: usize, param_name: String, value: f32 },
}

/// Types of MIDI effects available to add
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiEffectType {
    Transpose,
    Quantize,
    Swing,
    Humanize,
    Chance,
    Echo,
    Arpeggiator,
    Harmonizer,
}

impl MidiEffectType {
    pub fn all() -> &'static [MidiEffectType] {
        &[
            MidiEffectType::Transpose,
            MidiEffectType::Quantize,
            MidiEffectType::Swing,
            MidiEffectType::Humanize,
            MidiEffectType::Chance,
            MidiEffectType::Echo,
            MidiEffectType::Arpeggiator,
            MidiEffectType::Harmonizer,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Transpose => "Transpose",
            Self::Quantize => "Quantize",
            Self::Swing => "Swing",
            Self::Humanize => "Humanize",
            Self::Chance => "Chance",
            Self::Echo => "Echo",
            Self::Arpeggiator => "Arpeggiator",
            Self::Harmonizer => "Harmonizer",
        }
    }

    pub fn create_effect(&self) -> MidiEffect {
        match self {
            Self::Transpose => MidiEffect::Transpose(Default::default()),
            Self::Quantize => MidiEffect::Quantize(Default::default()),
            Self::Swing => MidiEffect::Swing(Default::default()),
            Self::Humanize => MidiEffect::Humanize(Default::default()),
            Self::Chance => MidiEffect::Chance(Default::default()),
            Self::Echo => MidiEffect::Echo(Default::default()),
            Self::Arpeggiator => MidiEffect::Arpeggiator(Default::default()),
            Self::Harmonizer => MidiEffect::Harmonizer(Default::default()),
        }
    }
}

/// MIDI FX rack panel state
pub struct MidiFxRackPanel {
    expanded_effect: Option<usize>,
    add_effect_type: MidiEffectType,
    drag_source: Option<usize>,
}

impl MidiFxRackPanel {
    pub fn new() -> Self {
        Self {
            expanded_effect: None,
            add_effect_type: MidiEffectType::Transpose,
            drag_source: None,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        track_name: Option<&str>,
        chain: Option<&MidiFxChain>,
    ) -> MidiFxRackAction {
        let mut action = MidiFxRackAction::None;

        ui.horizontal(|ui| {
            ui.heading("MIDI FX");
            if let Some(name) = track_name {
                ui.separator();
                ui.label(name);
            }
        });

        ui.separator();

        let Some(chain) = chain else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a MIDI track to view MIDI FX");
            });
            return action;
        };

        // Effect slots (vertical list, compact)
        for (idx, effect) in chain.effects.iter().enumerate() {
            let effect_action = self.draw_effect_slot(ui, idx, effect);
            if !matches!(effect_action, MidiFxRackAction::None) {
                action = effect_action;
            }
        }

        // Add effect section (compact)
        if chain.effects.len() < 8 {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ComboBox::from_label("")
                    .selected_text(self.add_effect_type.name())
                    .show_ui(ui, |ui| {
                        for effect_type in MidiEffectType::all() {
                            ui.selectable_value(&mut self.add_effect_type, *effect_type, effect_type.name());
                        }
                    });

                if ui.small_button("+").clicked() {
                    action = MidiFxRackAction::AddEffect(self.add_effect_type);
                }
            });
        }

        if chain.effects.is_empty() {
            ui.small("No FX");
        }

        action
    }

    fn draw_effect_slot(&mut self, ui: &mut Ui, idx: usize, effect: &MidiEffect) -> MidiFxRackAction {
        let mut action = MidiFxRackAction::None;
        let is_expanded = self.expanded_effect == Some(idx);
        let is_bypassed = effect.is_bypassed();

        // Compact effect header
        let header_height = 24.0;
        let (response, painter) = ui.allocate_painter(
            Vec2::new(ui.available_width(), header_height),
            Sense::click_and_drag(),
        );
        let rect = response.rect;

        // Background
        let bg_color = if is_bypassed {
            Color32::from_gray(35)
        } else {
            Color32::from_rgb(50, 60, 80)
        };
        painter.rect_filled(rect, 4.0, bg_color);
        painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_gray(70)), egui::StrokeKind::Outside);

        // Slot number
        painter.text(
            egui::pos2(rect.left() + 8.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{}", idx + 1),
            egui::FontId::proportional(10.0),
            Color32::from_gray(120),
        );

        // Effect name
        painter.text(
            egui::pos2(rect.left() + 24.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            effect.name(),
            egui::FontId::proportional(12.0),
            if is_bypassed { Color32::from_gray(100) } else { Color32::WHITE },
        );

        // Expand indicator
        let expand_icon = if is_expanded { "▼" } else { "▶" };
        painter.text(
            egui::pos2(rect.right() - 60.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            expand_icon,
            egui::FontId::proportional(10.0),
            Color32::from_gray(150),
        );

        // Bypass button
        let bypass_rect = Rect::from_center_size(
            egui::pos2(rect.right() - 36.0, rect.center().y),
            Vec2::new(20.0, 20.0),
        );
        let bypass_color = if is_bypassed {
            Color32::from_rgb(200, 150, 50)
        } else {
            Color32::from_gray(60)
        };
        painter.rect_filled(bypass_rect, 2.0, bypass_color);
        painter.text(
            bypass_rect.center(),
            egui::Align2::CENTER_CENTER,
            "B",
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );

        let bypass_response = ui.allocate_rect(bypass_rect, Sense::click());
        if bypass_response.clicked() {
            action = MidiFxRackAction::ToggleBypass(idx);
        }

        // Remove button
        let remove_rect = Rect::from_center_size(
            egui::pos2(rect.right() - 12.0, rect.center().y),
            Vec2::new(20.0, 20.0),
        );
        painter.rect_filled(remove_rect, 2.0, Color32::from_gray(60));
        painter.text(
            remove_rect.center(),
            egui::Align2::CENTER_CENTER,
            "×",
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );

        let remove_response = ui.allocate_rect(remove_rect, Sense::click());
        if remove_response.clicked() {
            action = MidiFxRackAction::RemoveEffect(idx);
        }

        // Toggle expanded on click
        if response.clicked() {
            self.expanded_effect = if is_expanded { None } else { Some(idx) };
        }

        // Drag and drop
        if response.drag_started() {
            self.drag_source = Some(idx);
        }

        if response.hovered() && ui.input(|i| i.pointer.any_released()) {
            if let Some(from) = self.drag_source.take() {
                if from != idx {
                    action = MidiFxRackAction::MoveEffect { from, to: idx };
                }
            }
        }

        // Parameter panel (when expanded)
        if is_expanded {
            ui.add_space(4.0);
            let params = effect.get_params();
            let param_action = self.draw_params(ui, idx, params);
            if !matches!(param_action, MidiFxRackAction::None) {
                action = param_action;
            }
            ui.add_space(4.0);
        }

        action
    }

    fn draw_params(&mut self, ui: &mut Ui, effect_idx: usize, params: &[MidiFxParam]) -> MidiFxRackAction {
        let mut action = MidiFxRackAction::None;

        ui.vertical(|ui| {
            ui.add_space(4.0);
            for param in params {
                ui.horizontal(|ui| {
                    ui.add_space(16.0); // Indent
                    ui.label(&param.name);
                    let mut value = param.value;
                    let slider = Slider::new(&mut value, param.min..=param.max)
                        .show_value(true)
                        .clamp_to_range(true);

                    if ui.add(slider).changed() {
                        action = MidiFxRackAction::SetParam {
                            effect_idx,
                            param_name: param.name.clone(),
                            value,
                        };
                    }
                });
            }
        });

        action
    }
}

impl Default for MidiFxRackPanel {
    fn default() -> Self {
        Self::new()
    }
}
