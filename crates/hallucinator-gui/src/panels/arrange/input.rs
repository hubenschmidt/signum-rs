use egui::{Rect, Ui};

use super::types::{ArrangeAction, ArrangeContext, LoopEdge};
use super::ArrangePanel;

const EDGE_HIT_WIDTH: f32 = 6.0;

impl ArrangePanel {
    pub(super) fn handle_loop_drag(
        &mut self,
        response: &egui::Response,
        ctx: &ArrangeContext,
        action: &mut ArrangeAction,
        ctrl_held: bool,
    ) {
        // Start drag — only with Ctrl held
        if response.drag_started() && ctrl_held {
            let Some(pos) = response.interact_pointer_pos() else { return };
            if !ctx.rect.contains(pos) { return };

            let beat = ctx.start_beat + (pos.x - ctx.rect.left()) / self.pixels_per_beat;
            let snapped = (beat / ctx.grid_step).floor() * ctx.grid_step;
            self.loop_drag_start = Some(snapped);
            self.loop_selection = Some((snapped, snapped + ctx.grid_step));
        }

        // Continue drag
        if response.dragged() {
            let Some(drag_start) = self.loop_drag_start else { return };
            let Some(pos) = response.interact_pointer_pos() else { return };

            let beat = ctx.start_beat + (pos.x - ctx.rect.left()) / self.pixels_per_beat;
            let snapped = (beat / ctx.grid_step).floor() * ctx.grid_step;

            let (sel_start, sel_end) = if snapped < drag_start {
                (snapped, drag_start)
            } else {
                (drag_start, snapped.max(drag_start + ctx.grid_step))
            };
            self.loop_selection = Some((sel_start, sel_end));
        }

        // End drag — set loop region
        if response.drag_stopped() && self.loop_drag_start.is_some() {
            if let Some((sel_start, sel_end)) = self.loop_selection {
                let start_sample = (sel_start as f64 * ctx.samples_per_beat) as u64;
                let end_sample = (sel_end as f64 * ctx.samples_per_beat) as u64;
                *action = ArrangeAction::SetLoopRegion { start_sample, end_sample };
            }
            self.loop_drag_start = None;
            self.loop_selection = None;
        }
    }

    pub(super) fn handle_click_to_seek(
        &self,
        response: &egui::Response,
        ctx: &ArrangeContext,
        action: &mut ArrangeAction,
    ) {
        if !response.clicked() || self.loop_drag_start.is_some() {
            return;
        }
        let Some(pos) = response.interact_pointer_pos() else { return };

        let mut click_beat = ctx.start_beat + (pos.x - ctx.rect.left()) / self.pixels_per_beat;
        if self.snap_to_grid {
            click_beat = (click_beat / ctx.grid_step).round() * ctx.grid_step;
        }
        *action = ArrangeAction::Seek((click_beat as f64 * ctx.samples_per_beat) as u64);
    }

    pub(super) fn handle_context_menu(
        response: &egui::Response,
        action: &mut ArrangeAction,
    ) {
        response.context_menu(|ui| {
            if ui.button("Add Audio Track").clicked() {
                *action = ArrangeAction::AddAudioTrack;
                ui.close_menu();
            }
            if ui.button("Add MIDI Track").clicked() {
                *action = ArrangeAction::AddMidiTrack;
                ui.close_menu();
            }
        });
    }

    /// Check if pointer is near a loop edge; returns which edge
    pub(super) fn detect_loop_edge(
        &self,
        ctx: &ArrangeContext,
        loop_start: u64,
        loop_end: u64,
        pointer_pos: egui::Pos2,
    ) -> Option<LoopEdge> {
        let loop_start_beat = loop_start as f64 / ctx.samples_per_beat;
        let loop_end_beat = loop_end as f64 / ctx.samples_per_beat;

        let start_x = ctx.rect.left() + ((loop_start_beat as f32 - ctx.start_beat) * self.pixels_per_beat);
        let end_x = ctx.rect.left() + ((loop_end_beat as f32 - ctx.start_beat) * self.pixels_per_beat);

        // Check horizontal proximity to edges (no vertical restriction)
        if (pointer_pos.x - start_x).abs() <= EDGE_HIT_WIDTH {
            return Some(LoopEdge::Start);
        }
        if (pointer_pos.x - end_x).abs() <= EDGE_HIT_WIDTH {
            return Some(LoopEdge::End);
        }
        None
    }

    pub(super) fn handle_loop_edge_drag(
        &mut self,
        ui: &egui::Ui,
        ctx: &ArrangeContext,
        action: &mut ArrangeAction,
        loop_start: u64,
        loop_end: u64,
    ) -> bool {
        let loop_start_beat = (loop_start as f64 / ctx.samples_per_beat) as f32;
        let loop_end_beat = (loop_end as f64 / ctx.samples_per_beat) as f32;

        let pointer = ui.input(|i| (i.pointer.hover_pos(), i.pointer.primary_down(), i.pointer.primary_released()));
        let (hover_pos, primary_down, primary_released) = pointer;

        // Try to start new edge drag when mouse pressed near edge
        let should_start = self.loop_edge_drag.is_none() && primary_down;
        let detected_edge = should_start
            .then(|| hover_pos)
            .flatten()
            .and_then(|pos| self.detect_loop_edge(ctx, loop_start, loop_end, pos));

        let new_drag = detected_edge.map(|edge| {
            let anchor = match edge {
                LoopEdge::Start => loop_end_beat,
                LoopEdge::End => loop_start_beat,
            };
            tracing::debug!("Loop edge drag started: {:?}", edge);
            (edge, anchor)
        });
        self.loop_edge_drag = self.loop_edge_drag.or(new_drag);

        let Some((edge, anchor)) = self.loop_edge_drag else { return false };
        let Some(pos) = hover_pos else { return true };

        let beat = ctx.start_beat + (pos.x - ctx.rect.left()) / self.pixels_per_beat;
        let snapped = (beat / ctx.grid_step).round() * ctx.grid_step;

        let (new_start, new_end) = match edge {
            LoopEdge::Start => (snapped.max(0.0).min(anchor - ctx.grid_step), anchor),
            LoopEdge::End => (anchor, snapped.max(anchor + ctx.grid_step)),
        };

        self.loop_selection = Some((new_start, new_end));

        if !primary_released {
            return true;
        }

        tracing::debug!("Loop edge drag stopped: {}..{}", new_start, new_end);
        let start_sample = (new_start as f64 * ctx.samples_per_beat) as u64;
        let end_sample = (new_end as f64 * ctx.samples_per_beat) as u64;
        *action = ArrangeAction::SetLoopRegion { start_sample, end_sample };
        self.loop_edge_drag = None;
        self.loop_selection = None;
        true
    }

    pub(super) fn handle_scroll_zoom(&mut self, ui: &Ui, rect: Rect) {
        let pointer_in_rect = ui.ctx().input(|i| {
            i.pointer.hover_pos().map(|p| rect.contains(p)).unwrap_or(false)
        });

        if !pointer_in_rect {
            return;
        }

        let (scroll_y, modifiers) = ui.ctx().input(|i| {
            let mut scroll = 0.0_f32;
            for event in &i.events {
                if let egui::Event::MouseWheel { delta, .. } = event {
                    scroll += delta.y;
                }
            }
            scroll += i.smooth_scroll_delta.y + i.raw_scroll_delta.y;
            (scroll, i.modifiers)
        });

        if scroll_y.abs() > 0.1 {
            if modifiers.ctrl || modifiers.command {
                let zoom_factor = 1.0 + scroll_y * 0.02;
                self.pixels_per_beat = (self.pixels_per_beat * zoom_factor).clamp(10.0, 200.0);
            } else {
                self.scroll_offset_beats = (self.scroll_offset_beats - scroll_y / self.pixels_per_beat).max(0.0);
            }
        }

        let scroll_x = ui.ctx().input(|i| i.smooth_scroll_delta.x + i.raw_scroll_delta.x);
        if scroll_x.abs() > 0.1 {
            self.scroll_offset_beats = (self.scroll_offset_beats - scroll_x / self.pixels_per_beat).max(0.0);
        }
    }
}
