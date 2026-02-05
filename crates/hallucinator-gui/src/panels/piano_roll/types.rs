/// Actions returned from piano roll
#[derive(Clone, Debug)]
pub enum PianoRollAction {
    None,
    ClipModified,
    /// Set loop region from selection
    SetLoopRegion {
        start_sample: u64,
        end_sample: u64,
    },
    /// Play a preview note (when not playing)
    PlayNote {
        pitch: u8,
        velocity: u8,
    },
    /// Stop a preview note
    StopNote {
        pitch: u8,
    },
    /// Stop multiple notes (used when deleting notes during playback)
    StopNotes {
        pitches: Vec<u8>,
    },
    /// Record a note during live recording
    RecordNote {
        pitch: u8,
        velocity: u8,
        start_tick: u64,
        duration_ticks: u64,
    },
}

/// State for dragging a note
#[derive(Clone, Copy)]
pub(super) enum DragMode {
    Move,
    ResizeEnd,
}

/// Note drag state
#[derive(Clone)]
pub(super) struct NoteDragState {
    pub note_idx: usize,
    pub mode: DragMode,
    pub original_start_tick: u64,
    pub original_duration_ticks: u64,
    pub original_pitch: u8,
    pub drag_start_beat: f64,
    pub drag_start_pitch: u8,
}

/// Loop selection state
#[derive(Clone)]
pub(super) struct LoopSelection {
    pub start_beat: f64,
    pub end_beat: f64,
}

/// What part of the loop is being dragged
#[derive(Clone, Copy)]
pub(super) enum LoopDragMode {
    Start,
    End,
    Move,
}
