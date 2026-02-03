//! signum-core: Domain types for the signum DAW

mod clip;
mod error;
mod timeline;
mod track;
mod transport;

pub use clip::{AudioClip, ClipId, MidiClip, MidiNote};
pub use error::{SignumError, Result};
pub use timeline::Timeline;
pub use track::{Track, TrackId, TrackKind};
pub use transport::{Transport, TransportState};
