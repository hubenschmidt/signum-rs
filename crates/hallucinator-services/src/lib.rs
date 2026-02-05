//! hallucinator-services: Audio engine, effects, and service layer

pub mod audio_effects;
pub mod audio_engine;
pub mod audio_input;
pub mod audio_io;
pub mod input_monitor;
pub mod wav_reader;

pub use audio_effects::{AudioEffect, EffectChain, EffectParam, Instrument, SampleKit, Sampler};
pub use audio_effects::{GainEffect, HighPassEffect, LowPassEffect, CompressorEffect, DelayEffect, ReverbEffect};
pub use audio_effects::{
    NativeWindowHandle, PluginGuiManager, Vst3Effect, Vst3Error, Vst3GuiError,
    Vst3Instrument, Vst3PluginInfo, Vst3Scanner,
};
pub use audio_effects::{
    Drum808, KICK, RIM_SHOT, SNARE, CLAP, CLOSED_HAT, OPEN_HAT, LOW_TOM,
    MID_TOM, HIGH_TOM, CRASH, COWBELL, HI_CONGA, MID_CONGA, LOW_CONGA, MARACAS, CLAVES,
};
pub use audio_engine::{AudioEngine, AudioEngineError, DrumPattern, DrumPatternStep, EngineState};
pub use audio_input::{AudioInputService, AudioInputError, InputDevice};
pub use audio_io::{AudioOutputService, AudioOutputError};
pub use input_monitor::{InputMonitor, MeterState, MonitorError, RecordedAudio};
