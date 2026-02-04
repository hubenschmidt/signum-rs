//! signum-core: Domain types for the signum DAW

pub mod algorithms;
mod clip;
mod error;
pub mod midi_fx;
pub mod pattern;
pub mod song;
mod timeline;
mod track;
mod transport;

pub use algorithms::{
    euclidean_rhythm, quantize_to_scale, scale_notes,
    ChordGenerator, ChordQuality, ScaleMode, Voicing,
};
pub use clip::{AudioClip, ClipId, MidiClip, MidiNote};
pub use error::{SignumError, Result};
pub use midi_fx::{MidiEffect, MidiEvent, MidiFx, MidiFxChain, MidiFxParam};
pub use midi_fx::{TransposeFx, QuantizeFx, SwingFx, HumanizeFx, ChanceFx, EchoFx, ArpeggiatorFx, HarmonizerFx};
pub use pattern::{PatternBank, PatternSlot};
pub use song::{PlaybackMode, SongArrangement, SongSection};
pub use timeline::Timeline;
pub use track::{Track, TrackId, TrackKind};
pub use transport::{Transport, TransportState};
