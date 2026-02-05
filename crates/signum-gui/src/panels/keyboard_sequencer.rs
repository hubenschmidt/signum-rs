//! QWERTY keyboard sequencer - Hapax-style pad grid with scale-aware keyboard

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use egui::{Color32, Key, Rect, Sense, Stroke, Ui, Vec2};
use signum_core::ScaleMode;

use crate::clipboard::{ClipboardContent, DawClipboard};

/// Drum step keys (top row: 1-0, -, =)
const DRUM_KEYS: [Key; 12] = [
    Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6,
    Key::Num7, Key::Num8, Key::Num9, Key::Num0, Key::Minus, Key::Plus,
];

/// Octave 3 keys (Q row)
const OCTAVE_3_KEYS: [Key; 12] = [
    Key::Q, Key::W, Key::E, Key::R, Key::T, Key::Y,
    Key::U, Key::I, Key::O, Key::P, Key::OpenBracket, Key::CloseBracket,
];

/// Octave 4 keys (A row)
const OCTAVE_4_KEYS: [Key; 12] = [
    Key::A, Key::S, Key::D, Key::F, Key::G, Key::H,
    Key::J, Key::K, Key::L, Key::Semicolon, Key::Quote, Key::Backslash,
];

/// Octave 5 keys (Z row)
const OCTAVE_5_KEYS: [Key; 12] = [
    Key::Z, Key::X, Key::C, Key::V, Key::B, Key::N,
    Key::M, Key::Comma, Key::Period, Key::Slash, Key::Questionmark, Key::Enter,
];

/// Note names for display
const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Which notes are black keys (sharps/flats)
const IS_BLACK_KEY: [bool; 12] = [false, true, false, true, false, false, true, false, true, false, true, false];

/// Drum key labels
const DRUM_KEY_LABELS: [&str; 12] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "="];

/// All scale modes for selector
const ALL_SCALES: [ScaleMode; 12] = [
    ScaleMode::Chromatic, ScaleMode::Major, ScaleMode::Minor,
    ScaleMode::Dorian, ScaleMode::Phrygian, ScaleMode::Lydian,
    ScaleMode::Mixolydian, ScaleMode::Locrian, ScaleMode::HarmonicMinor,
    ScaleMode::MelodicMinor, ScaleMode::Pentatonic, ScaleMode::Blues,
];

// -- Hapax color palette --
const PAD_BG: Color32 = Color32::from_rgb(38, 38, 42);
const PAD_BORDER: Color32 = Color32::from_rgb(55, 55, 60);
const PAD_ACTIVE: Color32 = Color32::from_rgb(220, 195, 90);       // warm yellow (lit pad)
const PAD_ACTIVE_STEP: Color32 = Color32::from_rgb(255, 235, 130); // bright glow (active+current)
const PAD_CURRENT: Color32 = Color32::from_rgb(70, 68, 50);        // dim highlight (step cursor)
const PAD_PRESSED: Color32 = Color32::from_rgb(140, 200, 240);     // melodic key held
const PAD_BLACK: Color32 = Color32::from_rgb(28, 28, 32);          // chromatic black key
const PANEL_BG: Color32 = Color32::from_rgb(22, 22, 26);           // dark metal background
const LABEL_DIM: Color32 = Color32::from_rgb(120, 120, 130);
const LABEL_BRIGHT: Color32 = Color32::from_rgb(210, 210, 215);
/// Layout sizes for docked vs floating mode
struct PadLayout {
    size: f32,
    spacing: f32,
    radius: f32,
    label_w: f32,
    font_pad: f32,
    font_header: f32,
    header_h: f32,
    glow_inset: f32,
}

const DOCKED: PadLayout = PadLayout {
    size: 28.0, spacing: 2.0, radius: 4.0, label_w: 26.0,
    font_pad: 10.0, font_header: 9.0, header_h: 14.0, glow_inset: 3.0,
};
const FLOATING: PadLayout = PadLayout {
    size: 72.0, spacing: 5.0, radius: 8.0, label_w: 40.0,
    font_pad: 18.0, font_header: 14.0, header_h: 20.0, glow_inset: 5.0,
};

fn truncate_label(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    s[..max].to_string()
}

/// A single drum step with optional sample assignment
#[derive(Clone, Default)]
pub struct DrumStep {
    pub active: bool,
    pub sample_name: Option<String>,
}

/// Action returned from keyboard sequencer
#[derive(Clone)]
pub enum KeyboardSequencerAction {
    None,
    ToggleDrumStep(usize),
    PlayNote { pitch: u8, velocity: u8 },
    StopNote { pitch: u8 },
    LoadStepSample { step: usize, path: PathBuf },
    PlayDrumStep { step: usize, velocity: u8 },
    CopyDrumStep(usize),
    CopyStepSample { from: usize, to: usize },
    PasteStepSample { step: usize, name: String, data: Arc<Vec<f32>> },
}

/// Payload for dragging a drum step within the sequencer
#[derive(Clone)]
struct DragStep(usize);

/// QWERTY keyboard sequencer panel
pub struct KeyboardSequencerPanel {
    drum_steps: Vec<DrumStep>,
    pressed_keys: HashMap<Key, u8>,
    current_step: usize,
    elasticity_pct: f64,
    base_velocity: u8,
    scale_mode: ScaleMode,
    root_note: u8,
    pub is_floating: bool,
    selected_step: Option<usize>,
}

impl KeyboardSequencerPanel {
    fn layout(&self) -> &'static PadLayout {
        if self.is_floating { &FLOATING } else { &DOCKED }
    }

    pub fn new() -> Self {
        Self {
            drum_steps: vec![DrumStep::default(); 12],
            pressed_keys: HashMap::new(),
            current_step: 0,
            elasticity_pct: 0.0,
            base_velocity: 100,
            scale_mode: ScaleMode::Chromatic,
            root_note: 0,
            is_floating: false,
            selected_step: None,
        }
    }

    fn step_count(&self) -> usize {
        let len = self.scale_mode.intervals().len();
        if self.scale_mode == ScaleMode::Chromatic { return len; }
        len + 1
    }

    fn pitch_for_step(&self, base_pitch: u8, step: usize) -> u8 {
        let intervals = self.scale_mode.intervals();
        let root = if self.scale_mode == ScaleMode::Chromatic { 0 } else { self.root_note };
        if step >= intervals.len() { return base_pitch + root + 12; }
        base_pitch + root + intervals[step]
    }

    fn note_name_for_step(&self, step: usize) -> &'static str {
        let intervals = self.scale_mode.intervals();
        let root = if self.scale_mode == ScaleMode::Chromatic { 0 } else { self.root_note };
        let semitone = if step >= intervals.len() { 0 } else { intervals[step] };
        NOTE_NAMES[(root + semitone) as usize % 12]
    }

    fn is_step_accidental(&self, step: usize) -> bool {
        if self.scale_mode != ScaleMode::Chromatic { return false; }
        IS_BLACK_KEY[step % 12]
    }

    fn row_label(&self, base_pitch: u8) -> String {
        let root = if self.scale_mode == ScaleMode::Chromatic { 0 } else { self.root_note };
        let start = base_pitch + root;
        let octave = (start as i32 / 12) - 1;
        format!("{}{}", NOTE_NAMES[start as usize % 12], octave)
    }

    pub fn set_current_step(&mut self, step: usize) {
        self.current_step = step % self.step_count();
    }

    pub fn get_drum_steps(&self) -> &[DrumStep] {
        &self.drum_steps
    }

    /// Set the sample name for a drum step (called by app after loading)
    pub fn set_step_sample_name(&mut self, step: usize, name: String) {
        if step < self.drum_steps.len() {
            self.drum_steps[step].sample_name = Some(name);
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        track_name: Option<&str>,
        playback_position: u64,
        bpm: f64,
        sample_rate: u32,
        is_playing: bool,
        clipboard: &DawClipboard,
    ) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let sc = self.step_count();

        // Compute current step from playback position with elasticity
        if is_playing && sample_rate > 0 {
            let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
            let master_beat = playback_position as f64 / samples_per_beat;
            let elastic_beat = master_beat * (1.0 + self.elasticity_pct / 100.0);
            let new_step = (elastic_beat as usize) % sc;
            // Fire active drum steps when step cursor advances
            if new_step != self.current_step {
                let step = &self.drum_steps[new_step];
                if step.active && step.sample_name.is_some() {
                    actions.push(KeyboardSequencerAction::PlayDrumStep {
                        step: new_step,
                        velocity: self.base_velocity,
                    });
                }
            }
            self.current_step = new_step;
        }

        // Dark panel background
        let panel_rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(panel_rect, 0.0, PANEL_BG);

        // Compact toolbar: title + float + scale + phase + vel
        ui.horizontal(|ui| {
            ui.colored_label(LABEL_BRIGHT, "HAPAX");
            if let Some(name) = track_name {
                ui.colored_label(LABEL_DIM, name);
            }
            let float_label = if self.is_floating { "Dock" } else { "Float" };
            if ui.button(float_label).clicked() {
                self.is_floating = !self.is_floating;
            }
            ui.separator();
            egui::ComboBox::from_id_salt("scale_combo")
                .selected_text(self.scale_mode.name())
                .width(90.0)
                .show_ui(ui, |ui| {
                    for &mode in &ALL_SCALES {
                        ui.selectable_value(&mut self.scale_mode, mode, mode.name());
                    }
                });
            if self.scale_mode != ScaleMode::Chromatic {
                egui::ComboBox::from_id_salt("root_combo")
                    .selected_text(NOTE_NAMES[self.root_note as usize % 12])
                    .width(40.0)
                    .show_ui(ui, |ui| {
                        for (i, &name) in NOTE_NAMES.iter().enumerate() {
                            ui.selectable_value(&mut self.root_note, i as u8, name);
                        }
                    });
            }
            ui.separator();
            ui.colored_label(LABEL_DIM, "Ph");
            ui.add(egui::Slider::new(&mut self.elasticity_pct, -10.0..=10.0)
                .suffix("%")
                .fixed_decimals(1));
            ui.colored_label(LABEL_DIM, "Vel");
            let mut vel = self.base_velocity as f32;
            ui.add(egui::Slider::new(&mut vel, 1.0..=127.0).fixed_decimals(0));
            self.base_velocity = vel as u8;
        });

        // Resize drum steps if scale changed
        self.drum_steps.resize(sc, DrumStep::default());

        ui.add_space(2.0);

        // Handle keyboard input
        actions.extend(self.handle_drum_input(ui));
        actions.extend(self.handle_melodic_input(ui));
        actions.extend(self.handle_drum_copy_paste(ui, clipboard));

        // Pad grid
        let l = self.layout();
        let sp = l.spacing;
        let label_3 = self.row_label(48);
        let label_4 = self.row_label(60);
        let label_5 = self.row_label(72);
        ui.vertical(|ui| {
            actions.extend(self.draw_drum_row(ui, is_playing, clipboard));
            ui.add_space(sp);
            self.draw_melodic_row(ui, &label_3, &OCTAVE_3_KEYS[..sc], 48, is_playing);
            ui.add_space(sp);
            self.draw_melodic_row(ui, &label_4, &OCTAVE_4_KEYS[..sc], 60, is_playing);
            ui.add_space(sp);
            self.draw_melodic_row(ui, &label_5, &OCTAVE_5_KEYS[..sc], 72, is_playing);
        });

        actions
    }

    fn handle_drum_input(&mut self, ui: &mut Ui) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let sc = self.step_count();

        for (i, &key) in DRUM_KEYS[..sc].iter().enumerate() {
            if !ui.input(|inp| inp.key_pressed(key)) { continue; }
            self.drum_steps[i].active = !self.drum_steps[i].active;
            actions.push(KeyboardSequencerAction::ToggleDrumStep(i));
            // Preview sound when toggling ON a step that has a sample
            if self.drum_steps[i].active && self.drum_steps[i].sample_name.is_some() {
                actions.push(KeyboardSequencerAction::PlayDrumStep { step: i, velocity: self.base_velocity });
            }
        }

        actions
    }

    fn handle_drum_copy_paste(
        &mut self,
        ui: &mut Ui,
        clipboard: &DawClipboard,
    ) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let modifiers = ui.input(|i| i.modifiers);
        let ctrl = modifiers.ctrl || modifiers.mac_cmd;

        // Ctrl+C: copy selected step (raw key only — avoids intercepting Event::Copy
        // from other panels which would overwrite the clipboard)
        if ctrl && ui.input(|i| i.key_pressed(Key::C)) {
            if let Some(sel) = self.selected_step {
                if self.drum_steps[sel].sample_name.is_some() {
                    actions.push(KeyboardSequencerAction::CopyDrumStep(sel));
                }
            }
        }

        // Ctrl+V: paste from clipboard — check Event::Paste (platform Ctrl+V)
        // and raw key as fallback
        let paste = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))))
            || (ctrl && ui.input(|i| i.key_pressed(Key::V)));
        if paste {
            if let Some(to) = self.selected_step {
                actions.extend(self.paste_from_clipboard(clipboard, to));
            }
        }

        actions
    }

    fn paste_from_clipboard(
        &self,
        clipboard: &DawClipboard,
        to: usize,
    ) -> Vec<KeyboardSequencerAction> {
        let Some(content) = clipboard.content() else { return Vec::new() };
        match content {
            ClipboardContent::FilePath(path) => {
                vec![KeyboardSequencerAction::LoadStepSample { step: to, path: path.clone() }]
            }
            ClipboardContent::SampleData { name, data } => {
                vec![KeyboardSequencerAction::PasteStepSample {
                    step: to,
                    name: name.clone(),
                    data: Arc::clone(data),
                }]
            }
        }
    }

    fn handle_melodic_input(&mut self, ui: &mut Ui) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let sc = self.step_count();

        // Skip melodic input when Ctrl/Cmd is held — those combos are app shortcuts (Ctrl+C/V/X)
        let has_shortcut_mod = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
        if has_shortcut_mod {
            return actions;
        }

        let velocity = ui.input(|i| {
            if i.modifiers.shift { return 127u8; }
            self.base_velocity
        });

        let octave_rows: [(&[Key], u8); 3] = [
            (&OCTAVE_3_KEYS[..sc], 48),
            (&OCTAVE_4_KEYS[..sc], 60),
            (&OCTAVE_5_KEYS[..sc], 72),
        ];

        for (keys, base_pitch) in octave_rows {
            for (i, &key) in keys.iter().enumerate() {
                let pitch = self.pitch_for_step(base_pitch, i);
                let is_pressed = ui.input(|inp| inp.key_down(key));
                let was_pressed = self.pressed_keys.contains_key(&key);

                if is_pressed && !was_pressed {
                    self.pressed_keys.insert(key, pitch);
                    actions.push(KeyboardSequencerAction::PlayNote { pitch, velocity });
                    continue;
                }
                if !is_pressed && was_pressed {
                    let sent_pitch = self.pressed_keys.remove(&key).unwrap_or(pitch);
                    actions.push(KeyboardSequencerAction::StopNote { pitch: sent_pitch });
                }
            }
        }

        actions
    }

    fn draw_drum_row(&mut self, ui: &mut Ui, is_playing: bool, clipboard: &DawClipboard) -> Vec<KeyboardSequencerAction> {
        let mut actions = Vec::new();
        let l = self.layout();

        ui.horizontal(|ui| {
            ui.allocate_ui(Vec2::new(l.label_w, l.size), |ui| {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(LABEL_DIM, "DR");
                });
            });

            for i in 0..self.drum_steps.len() {
                let active = self.drum_steps[i].active;
                let has_sample = self.drum_steps[i].sample_name.is_some();
                let is_current = is_playing && self.current_step == i;
                let is_selected = self.selected_step == Some(i);
                let (response, painter) = ui.allocate_painter(Vec2::splat(l.size), Sense::click_and_drag());
                let rect = response.rect;

                // --- Drop: browser file ---
                let file_payload = egui::DragAndDrop::payload::<PathBuf>(ui.ctx());
                let step_payload = egui::DragAndDrop::payload::<DragStep>(ui.ctx());
                let any_drop_hover = response.hovered()
                    && (file_payload.is_some() || step_payload.is_some());

                if response.hovered() && ui.input(|inp| inp.pointer.any_released()) {
                    if let Some(path) = &file_payload {
                        actions.push(KeyboardSequencerAction::LoadStepSample {
                            step: i,
                            path: (**path).clone(),
                        });
                    }
                    if let Some(src) = &step_payload {
                        let from = src.0;
                        if from != i && self.drum_steps[from].sample_name.is_some() {
                            actions.push(KeyboardSequencerAction::CopyStepSample { from, to: i });
                        }
                    }
                }

                // --- Click: select step, toggle active ---
                if response.clicked() && !any_drop_hover {
                    self.selected_step = Some(i);
                    self.drum_steps[i].active = !active;
                    actions.push(KeyboardSequencerAction::ToggleDrumStep(i));
                }

                // --- Drag: initiate step drag if step has a sample ---
                if response.dragged() && has_sample {
                    egui::DragAndDrop::set_payload(ui.ctx(), DragStep(i));
                }

                // --- Right-click context menu ---
                response.context_menu(|ui| {
                    if has_sample {
                        if ui.button("Copy").clicked() {
                            actions.push(KeyboardSequencerAction::CopyDrumStep(i));
                            ui.close_menu();
                        }
                    }
                    let can_paste = clipboard.content().is_some();
                    if can_paste {
                        if ui.button("Paste").clicked() {
                            actions.extend(self.paste_from_clipboard(clipboard, i));
                            ui.close_menu();
                        }
                    }
                    if has_sample {
                        if ui.button("Clear").clicked() {
                            self.drum_steps[i].sample_name = None;
                            self.drum_steps[i].active = false;
                            actions.push(KeyboardSequencerAction::ToggleDrumStep(i));
                            ui.close_menu();
                        }
                    }
                });

                // --- Visual ---
                let bg = if any_drop_hover {
                    Color32::from_rgb(80, 120, 180)
                } else if is_selected {
                    Color32::from_rgb(60, 80, 110)
                } else {
                    match (active, is_current) {
                        (true, true) => PAD_ACTIVE_STEP,
                        (true, false) => PAD_ACTIVE,
                        (false, true) => PAD_CURRENT,
                        (false, false) => PAD_BG,
                    }
                };

                let label = self.drum_steps[i].sample_name.as_ref()
                    .map(|n| truncate_label(n, 4))
                    .unwrap_or_else(|| DRUM_KEY_LABELS[i].to_string());

                self.draw_pad(&painter, rect, bg, &label, active);

                // Selection border
                if is_selected {
                    painter.rect_stroke(rect, l.radius, Stroke::new(2.0, Color32::from_rgb(130, 170, 220)), egui::StrokeKind::Outside);
                }

                ui.add_space(l.spacing);
            }
        });

        actions
    }

    fn draw_melodic_row(&self, ui: &mut Ui, label: &str, keys: &[Key], _base_pitch: u8, is_playing: bool) {
        let l = self.layout();
        ui.horizontal(|ui| {
            ui.allocate_ui(Vec2::new(l.label_w, l.size), |ui| {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(LABEL_DIM, label);
                });
            });

            for (i, &key) in keys.iter().enumerate() {
                let is_pressed = self.pressed_keys.contains_key(&key);
                let is_black = self.is_step_accidental(i);
                let is_current = is_playing && self.current_step == i;

                let (response, painter) = ui.allocate_painter(Vec2::splat(l.size), Sense::click());
                let rect = response.rect;

                let bg = if is_pressed {
                    PAD_PRESSED
                } else if is_current {
                    PAD_CURRENT
                } else if is_black {
                    PAD_BLACK
                } else {
                    PAD_BG
                };

                let text_color = if is_pressed || is_current {
                    LABEL_BRIGHT
                } else if is_black {
                    Color32::from_gray(90)
                } else {
                    LABEL_DIM
                };

                painter.rect_filled(rect, l.radius, bg);
                painter.rect_stroke(rect, l.radius, Stroke::new(1.0, PAD_BORDER), egui::StrokeKind::Outside);

                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    self.note_name_for_step(i),
                    egui::FontId::proportional(l.font_pad),
                    text_color,
                );

                ui.add_space(l.spacing);
            }
        });
    }

    /// Draw a single Hapax-style pad
    fn draw_pad(&self, painter: &egui::Painter, rect: Rect, bg: Color32, label: &str, lit: bool) {
        let l = self.layout();
        painter.rect_filled(rect, l.radius, bg);
        painter.rect_stroke(rect, l.radius, Stroke::new(1.0, PAD_BORDER), egui::StrokeKind::Outside);

        if lit {
            let glow = rect.shrink(l.glow_inset);
            painter.rect_filled(glow, l.radius - 2.0, Color32::from_rgba_premultiplied(255, 240, 160, 40));
        }

        let text_color = if lit { Color32::from_rgb(40, 35, 20) } else { LABEL_DIM };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(l.font_pad),
            text_color,
        );
    }
}

impl Default for KeyboardSequencerPanel {
    fn default() -> Self {
        Self::new()
    }
}
