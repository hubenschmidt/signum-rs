//! Timeline panel with tracks and clips

/// Live recording preview data
pub struct RecordingPreview {
    pub samples: Vec<f32>,
    pub start_sample: u64,
}
