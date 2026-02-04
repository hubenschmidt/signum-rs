//! Native (non-VST) instruments

pub mod drum808;

pub use drum808::{
    Drum808, KICK, RIM_SHOT, SNARE, CLAP, CLOSED_HAT, OPEN_HAT, LOW_TOM,
    MID_TOM, HIGH_TOM, CRASH, COWBELL, HI_CONGA, MID_CONGA, LOW_CONGA, MARACAS, CLAVES,
};
