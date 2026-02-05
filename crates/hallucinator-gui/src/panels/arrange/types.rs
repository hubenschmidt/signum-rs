use egui::Rect;
use hallucinator_core::ClipId;

/// Which edge of the loop region is being dragged
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoopEdge {
    Start,
    End,
}

/// Action returned from arrange panel
#[derive(Clone)]
pub enum ArrangeAction {
    None,
    SelectClip { track_idx: usize, clip_id: ClipId },
    OpenClipEditor { track_idx: usize, clip_id: ClipId },
    DeleteClip { track_idx: usize, clip_id: ClipId },
    Seek(u64),
    AddAudioTrack,
    AddMidiTrack,
    SetLoopRegion { start_sample: u64, end_sample: u64 },
}

/// Shared layout/timing context for a single arrange panel frame.
/// Built once in `ui()`, passed by reference to all extracted methods.
pub(super) struct ArrangeContext {
    pub rect: Rect,
    pub ruler_rect: Rect,
    pub track_area_top: f32,
    pub start_beat: f32,
    pub end_beat: f32,
    pub samples_per_beat: f64,
    pub grid_step: f32,
    pub pixels_per_grid: f32,
    pub time_sig_num: u8,
}
