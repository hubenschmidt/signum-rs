//! UI panels

mod arrange;
mod browser;
mod clip_editor;
mod device_rack;
mod drum_roll;
mod keyboard_sequencer;
mod midi_fx_rack;
mod piano_roll;
mod plugins;
mod song_view;
mod timeline;
mod track_headers;
mod transport;

pub use arrange::{ArrangeAction, ArrangePanel};
pub use browser::{BrowserAction, BrowserPanel};
pub use clip_editor::ClipEditorPanel;
pub use device_rack::{DeviceInfo, DeviceRackAction, DeviceRackPanel};
pub use drum_roll::{DrumRollAction, DrumRollPanel};
pub use keyboard_sequencer::{KeyboardSequencerAction, KeyboardSequencerPanel};
pub use midi_fx_rack::{MidiFxRackAction, MidiFxRackPanel};
pub use piano_roll::PianoRollAction;
pub use plugins::{PluginAction, PluginBrowserPanel};
pub use song_view::{SongViewAction, SongViewPanel};
pub use timeline::RecordingPreview;
pub use track_headers::{TrackHeaderAction, TrackHeadersPanel};
pub use transport::{TransportAction, TransportPanel};
