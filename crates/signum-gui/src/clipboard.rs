//! App-wide DAW clipboard for copy/paste across panels

use std::path::PathBuf;
use std::sync::Arc;

/// Content that can live on the DAW clipboard.
/// Extend with new variants as more panels gain copy/paste support.
#[derive(Clone)]
pub enum ClipboardContent {
    /// A file path (e.g. WAV selected in the browser)
    FilePath(PathBuf),
    /// Loaded sample data (e.g. copied from a drum kit slot)
    SampleData { name: String, data: Arc<Vec<f32>> },
}

/// Shared clipboard that lives on `SignumApp`.
#[derive(Default)]
pub struct DawClipboard {
    content: Option<ClipboardContent>,
}

impl DawClipboard {
    pub fn copy(&mut self, content: ClipboardContent) {
        self.content = Some(content);
    }

    pub fn content(&self) -> Option<&ClipboardContent> {
        self.content.as_ref()
    }

    pub fn clear(&mut self) {
        self.content = None;
    }

    pub fn has_file(&self) -> bool {
        matches!(self.content, Some(ClipboardContent::FilePath(_)))
    }

    pub fn has_sample(&self) -> bool {
        matches!(self.content, Some(ClipboardContent::SampleData { .. }))
    }
}
