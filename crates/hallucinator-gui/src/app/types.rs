use hallucinator_core::ClipId;

/// Selected clip info
#[derive(Clone, Copy)]
pub enum SelectedClip {
    Audio { track_idx: usize, clip_id: ClipId },
    Midi { track_idx: usize, clip_id: ClipId },
}

/// Floating plugin window state
pub(super) struct PluginWindow {
    pub id: u64,
    pub title: String,
    pub plugin_path: String,
    pub plugin_uid: String,
    pub open: bool,
    pub native_window_created: bool,
}
