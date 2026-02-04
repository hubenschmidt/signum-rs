//! Song view panel - horizontal timeline of sections for arrangement

use egui::{Color32, Rect, Sense, Stroke, Ui, Vec2};
use signum_core::{PlaybackMode, SongSection};

/// Action returned from song view
#[derive(Clone)]
pub enum SongViewAction {
    None,
    SelectSection(usize),
    AddSection,
    RemoveSection(usize),
    DuplicateSection(usize),
    MoveSection { from: usize, to: usize },
    SetSectionLength { index: usize, bars: u8 },
    SetSectionRepeat { index: usize, count: u8 },
    SetPlaybackMode(PlaybackMode),
    JumpToSection(usize),
}

/// Song view panel state
pub struct SongViewPanel {
    selected_section: usize,
    drag_source: Option<usize>,
    scroll_offset: f32,
}

impl SongViewPanel {
    pub fn new() -> Self {
        Self {
            selected_section: 0,
            drag_source: None,
            scroll_offset: 0.0,
        }
    }

    pub fn selected_section(&self) -> usize {
        self.selected_section
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        sections: &[SongSection],
        current_section: usize,
        playback_mode: PlaybackMode,
    ) -> SongViewAction {
        let mut action = SongViewAction::None;

        // Header with mode toggle
        ui.horizontal(|ui| {
            ui.heading("Song");

            ui.separator();

            // Playback mode toggle
            let pattern_selected = playback_mode == PlaybackMode::Pattern;
            if ui.selectable_label(pattern_selected, "Pattern").clicked() {
                action = SongViewAction::SetPlaybackMode(PlaybackMode::Pattern);
            }
            if ui.selectable_label(!pattern_selected, "Song").clicked() {
                action = SongViewAction::SetPlaybackMode(PlaybackMode::Song);
            }

            ui.separator();

            if ui.button("+ Add Section").clicked() {
                action = SongViewAction::AddSection;
            }
        });

        ui.separator();

        // Section timeline (compact)
        let section_height = 40.0;
        let bar_width = 16.0;

        egui::ScrollArea::horizontal()
            .id_salt("song_timeline")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (idx, section) in sections.iter().enumerate() {
                        let is_playing = idx == current_section && playback_mode == PlaybackMode::Song;
                        let section_action = self.draw_section(ui, idx, section, section_height, bar_width, is_playing);
                        if !matches!(section_action, SongViewAction::None) {
                            action = section_action;
                        }

                        // Separator between sections
                        if idx < sections.len() - 1 {
                            ui.add_space(4.0);
                        }
                    }
                });
            });

        // Section editor (for selected section)
        if let Some(section) = sections.get(self.selected_section) {
            ui.add_space(8.0);
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(format!("Section {}", self.selected_section + 1));

                ui.separator();

                ui.label("Length:");
                let mut bars = section.length_bars;
                let length_slider = egui::Slider::new(&mut bars, 1..=64).suffix(" bars");
                if ui.add(length_slider).changed() {
                    action = SongViewAction::SetSectionLength {
                        index: self.selected_section,
                        bars,
                    };
                }

                ui.separator();

                ui.label("Repeat:");
                let mut repeat = section.repeat_count;
                let repeat_slider = egui::Slider::new(&mut repeat, 1..=16).suffix("x");
                if ui.add(repeat_slider).changed() {
                    action = SongViewAction::SetSectionRepeat {
                        index: self.selected_section,
                        count: repeat,
                    };
                }

                ui.separator();

                if ui.button("Duplicate").clicked() {
                    action = SongViewAction::DuplicateSection(self.selected_section);
                }

                if sections.len() > 1 && ui.button("Remove").clicked() {
                    action = SongViewAction::RemoveSection(self.selected_section);
                }
            });
        }

        action
    }

    fn draw_section(
        &mut self,
        ui: &mut Ui,
        idx: usize,
        section: &SongSection,
        height: f32,
        bar_width: f32,
        is_playing: bool,
    ) -> SongViewAction {
        let mut action = SongViewAction::None;

        let width = section.length_bars as f32 * bar_width;
        let is_selected = self.selected_section == idx;

        let (response, painter) = ui.allocate_painter(Vec2::new(width, height), Sense::click_and_drag());
        let rect = response.rect;

        // Background color based on section index (cycling colors)
        let section_colors = [
            Color32::from_rgb(70, 90, 120),
            Color32::from_rgb(90, 70, 100),
            Color32::from_rgb(70, 100, 80),
            Color32::from_rgb(100, 90, 70),
            Color32::from_rgb(80, 80, 100),
            Color32::from_rgb(100, 80, 80),
        ];
        let bg_color = section_colors[idx % section_colors.len()];

        painter.rect_filled(rect, 4.0, bg_color);

        // Border
        let border_color = if is_selected {
            Color32::from_rgb(150, 180, 220)
        } else {
            Color32::from_gray(60)
        };
        let border_width = if is_selected { 2.0 } else { 1.0 };
        painter.rect_stroke(rect, 4.0, Stroke::new(border_width, border_color), egui::StrokeKind::Outside);

        // Section number
        painter.text(
            egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
            egui::Align2::LEFT_TOP,
            format!("{}", idx + 1),
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );

        // Section name or length
        let display_text = if !section.name.is_empty() {
            section.name.clone()
        } else {
            format!("{}b", section.length_bars)
        };

        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &display_text,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );

        // Repeat indicator
        if section.repeat_count > 1 {
            painter.text(
                egui::pos2(rect.right() - 6.0, rect.top() + 4.0),
                egui::Align2::RIGHT_TOP,
                format!("{}x", section.repeat_count),
                egui::FontId::proportional(10.0),
                Color32::from_gray(180),
            );
        }

        // Bar markers
        for bar in 1..section.length_bars {
            let x = rect.left() + bar as f32 * bar_width;
            painter.line_segment(
                [egui::pos2(x, rect.bottom() - 4.0), egui::pos2(x, rect.bottom())],
                Stroke::new(1.0, Color32::from_gray(100)),
            );
        }

        // Handle click
        if response.clicked() {
            self.selected_section = idx;
            action = SongViewAction::SelectSection(idx);
        }

        // Double-click to jump to section
        if response.double_clicked() {
            action = SongViewAction::JumpToSection(idx);
        }

        // Drag and drop
        if response.drag_started() {
            self.drag_source = Some(idx);
        }

        if response.hovered() && ui.input(|i| i.pointer.any_released()) {
            if let Some(from) = self.drag_source.take() {
                if from != idx {
                    action = SongViewAction::MoveSection { from, to: idx };
                }
            }
        }

        // Playing indicator
        if is_playing {
            let indicator_rect = Rect::from_min_size(
                egui::pos2(rect.left(), rect.bottom() - 4.0),
                Vec2::new(width, 4.0),
            );
            painter.rect_filled(indicator_rect, 0.0, Color32::from_rgb(100, 200, 120));
        }

        action
    }
}

impl Default for SongViewPanel {
    fn default() -> Self {
        Self::new()
    }
}
