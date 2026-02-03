//! UI panels

mod arrange;
mod browser;
mod clip_editor;
mod device_rack;
mod piano_roll;
mod plugins;
mod timeline;
mod track_headers;
mod transport;

pub use arrange::{ArrangeAction, ArrangePanel};
pub use browser::{BrowserAction, BrowserPanel};
pub use clip_editor::ClipEditorPanel;
pub use device_rack::{DeviceInfo, DeviceRackAction, DeviceRackPanel};
pub use piano_roll::{PianoRollAction, PianoRollPanel};
pub use plugins::{PluginAction, PluginBrowserPanel};
pub use timeline::{RecordingPreview, TimelinePanel};
pub use track_headers::{TrackHeaderAction, TrackHeadersPanel};
pub use transport::{TransportAction, TransportPanel};
