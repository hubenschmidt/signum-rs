//! Transport state and controls

use serde::{Deserialize, Serialize};

/// Transport playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TransportState {
    #[default]
    Stopped,
    Playing,
    Recording,
    Paused,
}

/// Transport controls and position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transport {
    pub state: TransportState,
    /// Current position in samples
    pub position_samples: u64,
    /// Sample rate for time conversion
    pub sample_rate: u32,
    /// Tempo in BPM
    pub bpm: f64,
    /// Time signature numerator
    pub time_sig_num: u8,
    /// Time signature denominator
    pub time_sig_denom: u8,
    /// Loop enabled
    pub loop_enabled: bool,
    /// Loop start in samples
    pub loop_start: u64,
    /// Loop end in samples
    pub loop_end: u64,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            state: TransportState::Stopped,
            position_samples: 0,
            sample_rate: 44100,
            bpm: 120.0,
            time_sig_num: 4,
            time_sig_denom: 4,
            loop_enabled: false,
            loop_start: 0,
            loop_end: 0,
        }
    }
}

impl Transport {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            ..Default::default()
        }
    }

    pub fn play(&mut self) {
        self.state = TransportState::Playing;
    }

    pub fn stop(&mut self) {
        self.state = TransportState::Stopped;
        self.position_samples = 0;
    }

    pub fn pause(&mut self) {
        self.state = TransportState::Paused;
    }

    pub fn record(&mut self) {
        self.state = TransportState::Recording;
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.state, TransportState::Playing | TransportState::Recording)
    }

    /// Position in seconds
    pub fn position_secs(&self) -> f64 {
        self.position_samples as f64 / self.sample_rate as f64
    }

    /// Set position from seconds
    pub fn set_position_secs(&mut self, secs: f64) {
        self.position_samples = (secs * self.sample_rate as f64) as u64;
    }

    /// Advance position by given samples, handling loop
    pub fn advance(&mut self, samples: u64) {
        self.position_samples += samples;

        if self.loop_enabled && self.position_samples >= self.loop_end && self.loop_end > self.loop_start {
            self.position_samples = self.loop_start;
        }
    }

    /// Format position as MM:SS.mmm
    pub fn format_time(&self) -> String {
        let secs = self.position_secs();
        let mins = (secs / 60.0) as u32;
        let secs_rem = secs % 60.0;
        format!("{:02}:{:05.2}", mins, secs_rem)
    }

    /// Samples per beat at current tempo
    pub fn samples_per_beat(&self) -> f64 {
        self.sample_rate as f64 * 60.0 / self.bpm
    }

    /// Current beat number (0-indexed)
    pub fn current_beat(&self) -> f64 {
        self.position_samples as f64 / self.samples_per_beat()
    }
}
