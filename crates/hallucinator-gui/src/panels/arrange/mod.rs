//! Arrange panel - timeline grid with clips in bars:beats

mod drawing;
mod input;
mod types;

pub use types::ArrangeAction;
use types::{ArrangeContext, LoopEdge};

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::clipboard::DawClipboard;
use egui::{Rect, Sense, Ui, Vec2};
use hallucinator_core::ClipId;
use hallucinator_services::{AudioEngine, EngineState};

use super::timeline::RecordingPreview;

/// Arrange panel state
pub struct ArrangePanel {
    pub pixels_per_beat: f32,
    pub scroll_offset_beats: f32,
    pub track_height: f32,
    pub vertical_scroll: f32,
    pub snap_to_grid: bool,
    /// Loop selection drag state (start_beat when dragging)
    loop_drag_start: Option<f32>,
    /// Current loop selection being drawn (start_beat, end_beat)
    loop_selection: Option<(f32, f32)>,
    /// Which loop edge is being resized, plus the opposite edge's beat position
    loop_edge_drag: Option<(LoopEdge, f32)>,
}

impl Default for ArrangePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ArrangePanel {
    pub fn new() -> Self {
        Self {
            pixels_per_beat: 40.0,
            scroll_offset_beats: 0.0,
            track_height: 80.0,
            vertical_scroll: 0.0,
            snap_to_grid: true,
            loop_drag_start: None,
            loop_selection: None,
            loop_edge_drag: None,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        _engine: &AudioEngine,
        state: &Arc<EngineState>,
        selected_track_idx: Option<usize>,
        selected_clip: Option<(usize, ClipId)>,
        recording_preview: Option<RecordingPreview>,
        _clipboard: &DawClipboard,
    ) -> ArrangeAction {
        let mut action = ArrangeAction::None;

        let available_rect = ui.available_rect_before_wrap();
        let (response, painter) = ui.allocate_painter(available_rect.size(), Sense::click_and_drag());
        let rect = response.rect;

        let Ok(timeline) = state.timeline.lock() else {
            return action;
        };

        let sample_rate = timeline.transport.sample_rate as f64;
        let bpm = timeline.transport.bpm;
        let time_sig_num = timeline.transport.time_sig_num;
        let samples_per_beat = sample_rate * 60.0 / bpm;

        let ruler_height = 24.0;
        let start_beat = self.scroll_offset_beats;
        let beats_visible = rect.width() / self.pixels_per_beat;

        // Grid subdivision based on zoom level
        let subdivision = if self.pixels_per_beat >= 160.0 {
            8.0
        } else if self.pixels_per_beat >= 80.0 {
            4.0
        } else if self.pixels_per_beat >= 40.0 {
            2.0
        } else if self.pixels_per_beat >= 20.0 {
            1.0
        } else {
            0.0
        };
        let grid_step = if subdivision > 0.0 { 1.0 / subdivision } else { time_sig_num as f32 };

        let ctx = ArrangeContext {
            rect,
            ruler_rect: Rect::from_min_size(rect.min, Vec2::new(rect.width(), ruler_height)),
            track_area_top: rect.top() + ruler_height,
            start_beat,
            end_beat: start_beat + beats_visible,
            samples_per_beat,
            grid_step,
            pixels_per_grid: self.pixels_per_beat * grid_step,
            time_sig_num,
        };

        // Drawing layers (order matters)
        self.draw_track_backgrounds(&painter, &ctx, &timeline.tracks, selected_track_idx);
        self.draw_grid(&painter, &ctx);
        self.draw_ruler(&painter, &ctx);

        let clip_action = self.draw_clips(&painter, ui, &ctx, &timeline.tracks, selected_clip, &recording_preview);
        if !matches!(clip_action, ArrangeAction::None) {
            action = clip_action;
        }

        let loop_enabled = timeline.transport.loop_enabled;
        let loop_start = timeline.transport.loop_start;
        let loop_end = timeline.transport.loop_end;

        if loop_enabled {
            self.draw_loop_region(&painter, &ctx, loop_start, loop_end);
        }

        drop(timeline);

        // Playhead (after dropping lock — uses atomic)
        let position_samples = state.position.load(Ordering::SeqCst);
        self.draw_playhead(&painter, &ctx, position_samples);

        // Input handling
        let ctrl_held = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);

        // Loop edge resize takes priority over new loop selection
        let edge_dragging = loop_enabled && self.handle_loop_edge_drag(ui, &ctx, &mut action, loop_start, loop_end);

        // Show resize cursor when hovering or dragging loop edges
        let hovering_edge = ui.input(|i| i.pointer.hover_pos())
            .map(|pos| self.detect_loop_edge(&ctx, loop_start, loop_end, pos).is_some())
            .unwrap_or(false);
        if loop_enabled && (edge_dragging || hovering_edge) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        if !edge_dragging {
            self.handle_loop_drag(&response, &ctx, &mut action, ctrl_held);
        }
        self.draw_loop_selection_overlay(&painter, &ctx);
        self.handle_click_to_seek(&response, &ctx, &mut action);
        Self::handle_context_menu(&response, &mut action);
        self.handle_scroll_zoom(ui, rect);

        action
    }
}
