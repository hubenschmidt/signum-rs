//! Transport controls panel with integrated VU meter

use std::sync::Arc;
use std::sync::atomic::Ordering;

use egui::{Ui, RichText, Color32, Rect, Stroke, Vec2, Sense};
use signum_services::{AudioEngine, EngineState, InputMonitor, MeterState};

/// Actions that can be triggered from transport
pub enum TransportAction {
    None,
    StartRecording,
    StopRecording,
}

pub struct TransportPanel {
    bpm_text: String,
    display_peak: f32,
}

impl TransportPanel {
    pub fn new() -> Self {
        Self {
            bpm_text: "120.0".to_string(),
            display_peak: 0.0,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        engine: &AudioEngine,
        state: &Arc<EngineState>,
        monitor: &mut InputMonitor,
        meter_state: &Arc<MeterState>,
    ) -> TransportAction {
        let mut action = TransportAction::None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            let is_playing = engine.is_playing();
            let is_recording = monitor.is_recording();

            // Rewind
            if ui.button(RichText::new("\u{23EE}").size(20.0)).clicked() {
                engine.stop_playback();
            }

            // Play/Pause
            let play_text = if is_playing { "\u{23F8}" } else { "\u{25B6}" };
            if ui.button(RichText::new(play_text).size(20.0)).clicked() {
                if is_playing {
                    engine.pause();
                } else {
                    engine.play();
                }
            }

            // Stop
            if ui.button(RichText::new("\u{23F9}").size(20.0)).clicked() {
                engine.stop_playback();
                if is_recording {
                    action = TransportAction::StopRecording;
                }
            }

            // Record button
            let rec_color = if is_recording {
                Color32::from_rgb(255, 50, 50)
            } else {
                Color32::from_rgb(200, 80, 80)
            };
            let rec_btn = ui.button(RichText::new("\u{23FA}").size(20.0).color(rec_color));
            if rec_btn.clicked() {
                action = if is_recording {
                    TransportAction::StopRecording
                } else {
                    TransportAction::StartRecording
                };
            }
            rec_btn.on_hover_text(if is_recording { "Stop recording" } else { "Start recording" });

            ui.separator();

            // Time display
            let position_samples = state.position.load(Ordering::SeqCst);
            let sample_rate = engine.sample_rate();
            let secs = position_samples as f64 / sample_rate as f64;
            let mins = (secs / 60.0) as u32;
            let secs_rem = secs % 60.0;

            ui.monospace(format!("{:02}:{:05.2}", mins, secs_rem));

            // Recording indicator
            if is_recording {
                ui.label(RichText::new("REC").color(Color32::RED).strong());
            }

            ui.separator();

            // Monitor controls
            let is_monitoring = monitor.is_running();
            let monitor_pass = monitor.is_monitor_enabled();

            // Input meter toggle
            let meter_btn_text = if is_monitoring { "\u{1F534}" } else { "\u{26AA}" };
            let meter_btn = ui.button(RichText::new(meter_btn_text).size(16.0));
            if meter_btn.clicked() {
                if is_monitoring {
                    let _ = monitor.stop();
                } else {
                    let _ = monitor.start("default");
                }
            }
            meter_btn.on_hover_text("Toggle input metering");

            // Monitor pass-through toggle
            let pass_color = if monitor_pass && is_monitoring {
                Color32::from_rgb(100, 200, 100)
            } else {
                Color32::from_gray(150)
            };
            let pass_btn = ui.button(RichText::new("\u{1F50A}").size(16.0).color(pass_color));
            if pass_btn.clicked() && is_monitoring {
                monitor.set_monitor_enabled(!monitor_pass);
            }
            pass_btn.on_hover_text("Toggle monitor pass-through");

            // VU Meter
            self.draw_meter(ui, meter_state, is_monitoring);

            ui.separator();

            // BPM
            ui.label("BPM:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.bpm_text)
                    .desired_width(50.0)
            );

            if response.lost_focus() {
                if let Ok(bpm) = self.bpm_text.parse::<f64>() {
                    engine.with_timeline(|timeline| {
                        timeline.transport.bpm = bpm.clamp(20.0, 300.0);
                    });
                }
            }

            // Time signature
            let (num, denom) = engine.with_timeline(|t| {
                (t.transport.time_sig_num, t.transport.time_sig_denom)
            }).unwrap_or((4, 4));

            ui.label(format!("{}/{}", num, denom));

            ui.separator();

            ui.label(format!("{}Hz", sample_rate));

            // Status on right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let status = match (is_playing, is_monitoring, is_recording) {
                    (_, _, true) => "Recording",
                    (true, _, _) => "Playing",
                    (false, true, _) => "Monitoring",
                    (false, false, _) => "Stopped",
                };
                ui.label(status);
            });
        });

        action
    }

    fn draw_meter(&mut self, ui: &mut Ui, meter_state: &Arc<MeterState>, is_monitoring: bool) {
        let peak = meter_state.peak();
        let clipped = meter_state.is_clipped();

        let smoothing = 0.3;
        self.display_peak = self.display_peak * (1.0 - smoothing) + peak * smoothing;

        if !is_monitoring {
            self.display_peak *= 0.9;
        }

        let meter_width = 150.0;
        let meter_height = 14.0;

        let (response, painter) = ui.allocate_painter(
            Vec2::new(meter_width + 8.0, meter_height + 2.0),
            Sense::click(),
        );

        let meter_rect = Rect::from_min_size(
            response.rect.min + Vec2::new(1.0, 1.0),
            Vec2::new(meter_width, meter_height),
        );

        painter.rect_filled(meter_rect, 2.0, Color32::from_gray(25));

        let peak_db = Self::linear_to_db(self.display_peak);
        let peak_width = Self::db_to_width(peak_db, meter_rect.width());

        if peak_width > 0.0 {
            let x_12db = Self::db_to_width(-12.0, meter_rect.width());
            let x_6db = Self::db_to_width(-6.0, meter_rect.width());

            let green_end = peak_width.min(x_12db);
            if green_end > 0.0 {
                painter.rect_filled(
                    Rect::from_min_size(meter_rect.min, Vec2::new(green_end, meter_height)),
                    2.0,
                    Color32::from_rgb(50, 160, 50),
                );
            }

            if peak_width > x_12db {
                let yellow_end = peak_width.min(x_6db);
                painter.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(meter_rect.left() + x_12db, meter_rect.top()),
                        egui::pos2(meter_rect.left() + yellow_end, meter_rect.bottom()),
                    ),
                    0.0,
                    Color32::from_rgb(180, 160, 50),
                );
            }

            if peak_width > x_6db {
                painter.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(meter_rect.left() + x_6db, meter_rect.top()),
                        egui::pos2(meter_rect.left() + peak_width.min(meter_rect.width()), meter_rect.bottom()),
                    ),
                    0.0,
                    Color32::from_rgb(180, 50, 50),
                );
            }
        }

        painter.rect_stroke(meter_rect, 2.0, Stroke::new(1.0, Color32::from_gray(50)), egui::StrokeKind::Outside);

        if clipped {
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(meter_rect.right() + 2.0, meter_rect.top()),
                    Vec2::new(5.0, meter_height),
                ),
                2.0,
                Color32::RED,
            );
            if response.clicked() {
                meter_state.clear_clip();
            }
        }

        ui.label(format!("{:+.0}", peak_db));
    }

    fn linear_to_db(linear: f32) -> f32 {
        if linear < 0.00001 {
            return -60.0;
        }
        20.0 * linear.log10()
    }

    fn db_to_width(db: f32, max_width: f32) -> f32 {
        let normalized = (db + 60.0) / 60.0;
        normalized.clamp(0.0, 1.0) * max_width
    }
}
