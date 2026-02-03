//! signum-services: Audio engine, effects, and service layer

pub mod audio_effects;
pub mod audio_engine;
pub mod audio_input;
pub mod audio_io;
pub mod input_monitor;

pub use audio_effects::{AudioEffect, EffectChain, EffectParam};
pub use audio_effects::{GainEffect, HighPassEffect, LowPassEffect, CompressorEffect, DelayEffect, ReverbEffect};
pub use audio_effects::{
    NativeWindowHandle, PluginGuiManager, Vst3Effect, Vst3Error, Vst3GuiError,
    Vst3Instrument, Vst3PluginInfo, Vst3Scanner,
};
pub use audio_engine::{AudioEngine, AudioEngineError, EngineState};
pub use audio_input::{AudioInputService, AudioInputError, InputDevice};
pub use audio_io::{AudioOutputService, AudioOutputError};
pub use input_monitor::{InputMonitor, MeterState, MonitorError, RecordedAudio};
