//! TR-808 style drum synthesizer

use crate::audio_effects::{AudioInstrument, EffectParam};

/// MIDI note mappings for drum sounds (GM drum map compatible)
pub const KICK: u8 = 36;       // C1
pub const RIM_SHOT: u8 = 37;   // C#1
pub const SNARE: u8 = 38;      // D1
pub const CLAP: u8 = 39;       // D#1
pub const CLOSED_HAT: u8 = 42; // F#1
pub const LOW_TOM: u8 = 43;    // G1
pub const MID_TOM: u8 = 45;    // A1
pub const OPEN_HAT: u8 = 46;   // A#1
pub const HIGH_TOM: u8 = 47;   // B1
pub const CRASH: u8 = 49;      // C#2
pub const COWBELL: u8 = 56;    // G#2
pub const HI_CONGA: u8 = 62;   // D3
pub const MID_CONGA: u8 = 63;  // D#3
pub const LOW_CONGA: u8 = 64;  // E3
pub const MARACAS: u8 = 70;    // A#3
pub const CLAVES: u8 = 75;     // D#4

/// Drum sound types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumKind {
    Kick,
    RimShot,
    Snare,
    Clap,
    ClosedHat,
    OpenHat,
    LowTom,
    MidTom,
    HighTom,
    Crash,
    Cowbell,
    HiConga,
    MidConga,
    LowConga,
    Maracas,
    Claves,
}

impl DrumKind {
    fn from_pitch(pitch: u8) -> Option<Self> {
        match pitch {
            KICK => Some(Self::Kick),
            RIM_SHOT => Some(Self::RimShot),
            SNARE => Some(Self::Snare),
            CLAP => Some(Self::Clap),
            CLOSED_HAT => Some(Self::ClosedHat),
            OPEN_HAT => Some(Self::OpenHat),
            LOW_TOM => Some(Self::LowTom),
            MID_TOM => Some(Self::MidTom),
            HIGH_TOM => Some(Self::HighTom),
            CRASH => Some(Self::Crash),
            COWBELL => Some(Self::Cowbell),
            HI_CONGA => Some(Self::HiConga),
            MID_CONGA => Some(Self::MidConga),
            LOW_CONGA => Some(Self::LowConga),
            MARACAS => Some(Self::Maracas),
            CLAVES => Some(Self::Claves),
            _ => None,
        }
    }
}

/// State for a single drum voice
#[derive(Debug, Clone)]
struct DrumVoice {
    kind: DrumKind,
    active: bool,
    age: usize,

    // Oscillator state
    phase: f64,
    phase2: f64, // Secondary oscillator (for snare)

    // Envelope states
    amp_env: f64,
    pitch_env: f64,

    // Noise state (for snare, hat, clap)
    noise_env: f64,

    // Clap burst state
    burst_count: usize,
    burst_timer: usize,

    // Filter state (two-pole for bandpass)
    filter_state: f64,
    filter_state2: f64,

    // Voice parameters (from trigger)
    velocity: f32,
}

impl DrumVoice {
    fn new() -> Self {
        Self {
            kind: DrumKind::Kick,
            active: false,
            age: 0,
            phase: 0.0,
            phase2: 0.0,
            amp_env: 0.0,
            pitch_env: 0.0,
            noise_env: 0.0,
            burst_count: 0,
            burst_timer: 0,
            filter_state: 0.0,
            filter_state2: 0.0,
            velocity: 1.0,
        }
    }

    fn trigger(&mut self, kind: DrumKind, velocity: u8) {
        self.kind = kind;
        self.active = true;
        self.age = 0;
        self.phase = 0.0;
        self.phase2 = 0.0;
        self.amp_env = 1.0;
        self.pitch_env = 1.0;
        self.noise_env = 1.0;
        self.filter_state = 0.0;
        self.filter_state2 = 0.0;
        self.velocity = velocity as f32 / 127.0;

        // Clap-specific init
        if kind == DrumKind::Clap {
            self.burst_count = 4;
            self.burst_timer = 0;
        }
    }

    fn tick(&mut self, sample_rate: f32, params: &Drum808Params) -> f32 {
        if !self.active {
            return 0.0;
        }

        self.age += 1;
        let dt = 1.0 / sample_rate as f64;

        let sample = match self.kind {
            DrumKind::Kick => self.tick_kick(dt, params),
            DrumKind::RimShot => self.tick_rimshot(dt, params),
            DrumKind::Snare => self.tick_snare(dt, params),
            DrumKind::Clap => self.tick_clap(dt, params),
            DrumKind::ClosedHat => self.tick_hat(dt, params, true),
            DrumKind::OpenHat => self.tick_hat(dt, params, false),
            DrumKind::LowTom => self.tick_tom(dt, params, 80.0),
            DrumKind::MidTom => self.tick_tom(dt, params, 120.0),
            DrumKind::HighTom => self.tick_tom(dt, params, 160.0),
            DrumKind::Crash => self.tick_crash(dt, params),
            DrumKind::Cowbell => self.tick_cowbell(dt, params),
            DrumKind::HiConga => self.tick_conga(dt, params, 400.0),
            DrumKind::MidConga => self.tick_conga(dt, params, 300.0),
            DrumKind::LowConga => self.tick_conga(dt, params, 200.0),
            DrumKind::Maracas => self.tick_maracas(dt, params),
            DrumKind::Claves => self.tick_claves(dt, params),
        };

        // Check if voice is done
        if self.amp_env < 0.0001 && self.noise_env < 0.0001 {
            self.active = false;
        }

        sample as f32 * self.velocity
    }

    fn tick_kick(&mut self, dt: f64, params: &Drum808Params) -> f64 {
        // Base frequency with tune control (40-80 Hz range)
        let base_freq = 40.0 + params.kick_tune as f64 * 40.0;

        // Pitch envelope decay (fast - 20-50ms)
        let pitch_decay = 0.02 + (1.0 - params.kick_decay as f64) * 0.03;
        self.pitch_env *= (-dt / pitch_decay).exp();

        // Frequency with pitch sweep (4x at start)
        let freq = base_freq * (1.0 + self.pitch_env * 3.0);

        // Phase accumulation
        self.phase += freq * dt;
        let osc = (self.phase * std::f64::consts::TAU).sin();

        // Amplitude envelope (longer decay 100-500ms)
        let amp_decay = 0.1 + params.kick_decay as f64 * 0.4;
        self.amp_env *= (-dt / amp_decay).exp();

        osc * self.amp_env * params.kick_level as f64
    }

    fn tick_snare(&mut self, dt: f64, params: &Drum808Params) -> f64 {
        // Classic 808 snare: two tuned oscillators + bandpass noise

        // Primary tone (around 180 Hz) - the "body"
        let tone1_freq = 180.0 + params.snare_tune as f64 * 40.0;
        self.phase += tone1_freq * dt;
        let tone1 = (self.phase * std::f64::consts::TAU).sin();

        // Secondary tone (around 330 Hz) - adds "snap"
        let tone2_freq = 330.0 + params.snare_tune as f64 * 50.0;
        self.phase2 += tone2_freq * dt;
        let tone2 = (self.phase2 * std::f64::consts::TAU).sin();

        // Tone envelope - very fast decay (20-40ms)
        let tone_decay = 0.015 + params.snare_decay as f64 * 0.025;
        self.pitch_env *= (-dt / tone_decay).exp();

        // Noise component - the "snares" rattling
        let noise = fastrand::f64() * 2.0 - 1.0;

        // Tight bandpass filter around 1000-2500 Hz for snare rattle
        // Two-pole bandpass approximation
        let bp_freq = 0.15; // ~1500 Hz at 44.1kHz
        let bp_q = 0.7;
        self.filter_state += bp_freq * (noise - self.filter_state);
        self.filter_state2 += bp_freq * bp_q * (self.filter_state - self.filter_state2);
        let filtered_noise = self.filter_state - self.filter_state2;

        // Noise envelope - slightly longer than tone (80-150ms)
        let noise_decay = 0.06 + params.snare_decay as f64 * 0.09;
        self.noise_env *= (-dt / noise_decay).exp();

        // Mix: tones provide punch, noise provides character
        let tone_mix = params.snare_tone as f64;
        let tones = (tone1 * 0.6 + tone2 * 0.4) * self.pitch_env * tone_mix;
        let snares = filtered_noise * self.noise_env * 1.5 * (1.0 - tone_mix * 0.3);

        (tones + snares) * params.snare_level as f64
    }

    fn tick_clap(&mut self, dt: f64, params: &Drum808Params) -> f64 {
        // Classic 808 clap: multiple noise bursts simulating hands clapping
        // 4 quick bursts spaced ~15-20ms apart, then reverb tail

        // Burst phase - re-trigger noise envelope for each burst
        if self.burst_count > 0 {
            self.burst_timer += 1;
            // ~750 samples = ~17ms at 44.1kHz between bursts
            if self.burst_timer > 750 {
                self.burst_timer = 0;
                self.burst_count -= 1;
                self.noise_env = 0.9; // Re-trigger with slightly less energy
            }
        }

        // Noise source
        let noise = fastrand::f64() * 2.0 - 1.0;

        // Bandpass filter around 1000-2000 Hz (clap frequency range)
        let bp_freq = 0.12;
        self.filter_state += bp_freq * (noise - self.filter_state);
        let bandpassed = noise - self.filter_state * 1.8; // Emphasis on mid frequencies

        // During bursts: very fast decay (5ms), then reverb tail (150-300ms)
        let decay = if self.burst_count > 0 {
            0.004  // Fast burst decay
        } else {
            0.12 + params.clap_level as f64 * 0.08  // Reverb tail
        };
        self.noise_env *= (-dt / decay).exp();

        // Add slight saturation for that analog warmth
        let saturated = (bandpassed * 1.5).tanh();

        saturated * self.noise_env * params.clap_level as f64
    }

    fn tick_hat(&mut self, dt: f64, params: &Drum808Params, closed: bool) -> f64 {
        // Metallic noise (mix of square waves at non-harmonic frequencies)
        // Simplified: filtered white noise
        let noise = fastrand::f64() * 2.0 - 1.0;

        // Highpass filter (removes low frequencies for metallic sound)
        let hp_cutoff = 0.3;
        self.filter_state += hp_cutoff * (noise - self.filter_state);
        let highpassed = noise - self.filter_state;

        // Decay time: closed = 20-50ms, open = 200-500ms
        let decay = if closed {
            0.02 + params.hat_decay as f64 * 0.03
        } else {
            0.2 + params.hat_decay as f64 * 0.3
        };

        self.amp_env *= (-dt / decay).exp();

        highpassed * self.amp_env * params.hat_level as f64
    }

    fn tick_tom(&mut self, dt: f64, params: &Drum808Params, base_freq: f64) -> f64 {
        // Tune control shifts +/- 30%
        let freq = base_freq * (0.7 + params.tom_tune as f64 * 0.6);

        // Pitch envelope (slight drop)
        self.pitch_env *= (-dt / 0.05).exp();
        let current_freq = freq * (1.0 + self.pitch_env * 0.5);

        // Phase accumulation
        self.phase += current_freq * dt;
        let osc = (self.phase * std::f64::consts::TAU).sin();

        // Amplitude envelope (200-400ms)
        let decay = 0.2 + params.tom_decay as f64 * 0.2;
        self.amp_env *= (-dt / decay).exp();

        osc * self.amp_env * params.tom_level as f64
    }

    fn tick_rimshot(&mut self, dt: f64, params: &Drum808Params) -> f64 {
        // High-pitched click with short noise burst
        let freq = 500.0;
        self.phase += freq * dt;
        let tone = (self.phase * std::f64::consts::TAU).sin();

        // Very fast decay (10-20ms)
        self.amp_env *= (-dt / 0.015).exp();

        // Add some noise for attack
        let noise = fastrand::f64() * 2.0 - 1.0;
        self.noise_env *= (-dt / 0.005).exp();

        let mix = tone * 0.7 + noise * self.noise_env * 0.3;
        mix * self.amp_env * params.perc_level as f64
    }

    fn tick_crash(&mut self, dt: f64, params: &Drum808Params) -> f64 {
        // Long metallic noise (similar to open hat but longer)
        let noise = fastrand::f64() * 2.0 - 1.0;

        // Highpass for metallic character
        let hp_cutoff = 0.25;
        self.filter_state += hp_cutoff * (noise - self.filter_state);
        let highpassed = noise - self.filter_state;

        // Long decay (500ms - 2s)
        let decay = 0.5 + params.cymbal_decay as f64 * 1.5;
        self.amp_env *= (-dt / decay).exp();

        highpassed * self.amp_env * params.cymbal_level as f64
    }

    fn tick_cowbell(&mut self, dt: f64, params: &Drum808Params) -> f64 {
        // Two detuned square waves for metallic sound
        let freq1 = 560.0;
        let freq2 = 845.0;

        self.phase += freq1 * dt;
        let phase2 = self.phase * (freq2 / freq1);

        // Square waves (sign of sine)
        let sq1 = if (self.phase * std::f64::consts::TAU).sin() > 0.0 { 1.0 } else { -1.0 };
        let sq2 = if (phase2 * std::f64::consts::TAU).sin() > 0.0 { 1.0 } else { -1.0 };

        let mix = (sq1 + sq2) * 0.3;

        // Short decay (100-200ms)
        self.amp_env *= (-dt / 0.15).exp();

        mix * self.amp_env * params.perc_level as f64
    }

    fn tick_conga(&mut self, dt: f64, params: &Drum808Params, base_freq: f64) -> f64 {
        // Tuned membrane sound with pitch drop
        let freq = base_freq * (0.8 + params.conga_tune as f64 * 0.4);

        // Pitch envelope
        self.pitch_env *= (-dt / 0.03).exp();
        let current_freq = freq * (1.0 + self.pitch_env * 0.3);

        self.phase += current_freq * dt;
        let osc = (self.phase * std::f64::consts::TAU).sin();

        // Medium decay (150-300ms)
        let decay = 0.15 + params.conga_decay as f64 * 0.15;
        self.amp_env *= (-dt / decay).exp();

        osc * self.amp_env * params.conga_level as f64
    }

    fn tick_maracas(&mut self, dt: f64, params: &Drum808Params) -> f64 {
        // Short burst of highpassed noise
        let noise = fastrand::f64() * 2.0 - 1.0;

        // Strong highpass
        let hp_cutoff = 0.4;
        self.filter_state += hp_cutoff * (noise - self.filter_state);
        let highpassed = noise - self.filter_state;

        // Very short decay (20-50ms)
        self.amp_env *= (-dt / 0.03).exp();

        highpassed * self.amp_env * params.perc_level as f64 * 0.5
    }

    fn tick_claves(&mut self, dt: f64, params: &Drum808Params) -> f64 {
        // Sharp wooden click - high frequency sine with very fast decay
        let freq = 2500.0;
        self.phase += freq * dt;
        let osc = (self.phase * std::f64::consts::TAU).sin();

        // Very fast decay (5-15ms)
        self.amp_env *= (-dt / 0.01).exp();

        osc * self.amp_env * params.perc_level as f64
    }
}

/// Parameters for the 808 drum machine
#[derive(Debug, Clone)]
pub struct Drum808Params {
    pub master: f32,
    pub kick_level: f32,
    pub kick_tune: f32,
    pub kick_decay: f32,
    pub snare_level: f32,
    pub snare_tune: f32,
    pub snare_decay: f32,
    pub snare_tone: f32,
    pub hat_level: f32,
    pub hat_decay: f32,
    pub clap_level: f32,
    pub tom_level: f32,
    pub tom_tune: f32,
    pub tom_decay: f32,
    pub cymbal_level: f32,
    pub cymbal_decay: f32,
    pub conga_level: f32,
    pub conga_tune: f32,
    pub conga_decay: f32,
    pub perc_level: f32, // rimshot, cowbell, maracas, claves
}

impl Default for Drum808Params {
    fn default() -> Self {
        Self {
            master: 0.8,
            kick_level: 0.9,
            kick_tune: 0.5,
            kick_decay: 0.5,
            snare_level: 0.8,
            snare_tune: 0.5,
            snare_decay: 0.5,
            snare_tone: 0.4,
            hat_level: 0.6,
            hat_decay: 0.3,
            clap_level: 0.7,
            tom_level: 0.7,
            tom_tune: 0.5,
            tom_decay: 0.5,
            cymbal_level: 0.6,
            cymbal_decay: 0.6,
            conga_level: 0.7,
            conga_tune: 0.5,
            conga_decay: 0.5,
            perc_level: 0.7,
        }
    }
}

const MAX_VOICES: usize = 16;
const MAX_BLOCK_SIZE: usize = 4096;

/// TR-808 style drum synthesizer
pub struct Drum808 {
    sample_rate: f32,
    voices: Vec<DrumVoice>,
    pending_events: Vec<(u8, u8, u32)>, // (pitch, velocity, offset)
    output_left: Vec<f32>,
    output_right: Vec<f32>,
    params: Drum808Params,
    param_cache: Vec<EffectParam>,
}

impl std::fmt::Debug for Drum808 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Drum808")
            .field("sample_rate", &self.sample_rate)
            .field("active_voices", &self.voices.iter().filter(|v| v.active).count())
            .finish()
    }
}

impl Drum808 {
    pub fn new(sample_rate: f32) -> Self {
        let voices = (0..MAX_VOICES).map(|_| DrumVoice::new()).collect();
        let params = Drum808Params::default();
        let param_cache = Self::build_param_cache(&params);

        Self {
            sample_rate,
            voices,
            pending_events: Vec::new(),
            output_left: vec![0.0; MAX_BLOCK_SIZE],
            output_right: vec![0.0; MAX_BLOCK_SIZE],
            params,
            param_cache,
        }
    }

    fn build_param_cache(params: &Drum808Params) -> Vec<EffectParam> {
        vec![
            EffectParam::new("master", params.master, 0.0, 1.0, ""),
            EffectParam::new("kick_level", params.kick_level, 0.0, 1.0, ""),
            EffectParam::new("kick_tune", params.kick_tune, 0.0, 1.0, ""),
            EffectParam::new("kick_decay", params.kick_decay, 0.0, 1.0, ""),
            EffectParam::new("snare_level", params.snare_level, 0.0, 1.0, ""),
            EffectParam::new("snare_tune", params.snare_tune, 0.0, 1.0, ""),
            EffectParam::new("snare_decay", params.snare_decay, 0.0, 1.0, ""),
            EffectParam::new("snare_tone", params.snare_tone, 0.0, 1.0, ""),
            EffectParam::new("hat_level", params.hat_level, 0.0, 1.0, ""),
            EffectParam::new("hat_decay", params.hat_decay, 0.0, 1.0, ""),
            EffectParam::new("clap_level", params.clap_level, 0.0, 1.0, ""),
            EffectParam::new("tom_level", params.tom_level, 0.0, 1.0, ""),
            EffectParam::new("tom_tune", params.tom_tune, 0.0, 1.0, ""),
            EffectParam::new("tom_decay", params.tom_decay, 0.0, 1.0, ""),
        ]
    }

    fn update_param_cache(&mut self) {
        self.param_cache = Self::build_param_cache(&self.params);
    }

    fn trigger_drum(&mut self, pitch: u8, velocity: u8) {
        let Some(kind) = DrumKind::from_pitch(pitch) else { return };

        // Hi-hat choke: open hat kills closed, closed kills open
        if kind == DrumKind::ClosedHat || kind == DrumKind::OpenHat {
            for voice in &mut self.voices {
                if voice.active && (voice.kind == DrumKind::ClosedHat || voice.kind == DrumKind::OpenHat) {
                    voice.active = false;
                }
            }
        }

        // Find voice: same drum type (retrigger), or inactive, or oldest
        let voice_idx = self.voices.iter().position(|v| v.active && v.kind == kind)
            .or_else(|| self.voices.iter().position(|v| !v.active))
            .unwrap_or_else(|| {
                self.voices.iter()
                    .enumerate()
                    .max_by_key(|(_, v)| v.age)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            });

        self.voices[voice_idx].trigger(kind, velocity);
    }
}

impl AudioInstrument for Drum808 {
    fn name(&self) -> &str {
        "808 Drums"
    }

    fn queue_note_on(&mut self, pitch: u8, velocity: u8, _channel: u8, sample_offset: u32) {
        self.pending_events.push((pitch, velocity, sample_offset));
    }

    fn queue_note_off(&mut self, _pitch: u8, _velocity: u8, _channel: u8, _sample_offset: u32) {
        // Drums are one-shot, no note-off handling needed
    }

    fn all_notes_off(&mut self) {
        for voice in &mut self.voices {
            voice.active = false;
        }
    }

    fn process(&mut self, num_frames: usize) -> (&[f32], &[f32]) {
        let frames = num_frames.min(MAX_BLOCK_SIZE);

        self.output_left[..frames].fill(0.0);
        self.output_right[..frames].fill(0.0);

        // Sort events by offset
        self.pending_events.sort_by_key(|e| e.2);

        for frame_idx in 0..frames {
            // Trigger events at this frame
            while let Some(&(pitch, velocity, offset)) = self.pending_events.first() {
                if offset as usize > frame_idx {
                    break;
                }
                self.pending_events.remove(0);
                self.trigger_drum(pitch, velocity);
            }

            // Process all active voices
            let mut mix = 0.0f32;
            for voice in &mut self.voices {
                if !voice.active {
                    continue;
                }
                mix += voice.tick(self.sample_rate, &self.params);
            }

            // Apply master level and soft clip
            let out = (mix * self.params.master).tanh();
            self.output_left[frame_idx] = out;
            self.output_right[frame_idx] = out;
        }

        // Clear processed events
        self.pending_events.retain(|e| e.2 as usize >= frames);
        for event in &mut self.pending_events {
            event.2 -= frames as u32;
        }

        (&self.output_left[..frames], &self.output_right[..frames])
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    fn get_params(&self) -> &[EffectParam] {
        &self.param_cache
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "master" => self.params.master = value,
            "kick_level" => self.params.kick_level = value,
            "kick_tune" => self.params.kick_tune = value,
            "kick_decay" => self.params.kick_decay = value,
            "snare_level" => self.params.snare_level = value,
            "snare_tune" => self.params.snare_tune = value,
            "snare_decay" => self.params.snare_decay = value,
            "snare_tone" => self.params.snare_tone = value,
            "hat_level" => self.params.hat_level = value,
            "hat_decay" => self.params.hat_decay = value,
            "clap_level" => self.params.clap_level = value,
            "tom_level" => self.params.tom_level = value,
            "tom_tune" => self.params.tom_tune = value,
            "tom_decay" => self.params.tom_decay = value,
            _ => return,
        }
        self.update_param_cache();
    }

    fn set_param_by_index(&mut self, index: usize, value: f64) {
        let value = value as f32;
        match index {
            0 => self.params.master = value,
            1 => self.params.kick_level = value,
            2 => self.params.kick_tune = value,
            3 => self.params.kick_decay = value,
            4 => self.params.snare_level = value,
            5 => self.params.snare_tune = value,
            6 => self.params.snare_decay = value,
            7 => self.params.snare_tone = value,
            8 => self.params.hat_level = value,
            9 => self.params.hat_decay = value,
            10 => self.params.clap_level = value,
            11 => self.params.tom_level = value,
            12 => self.params.tom_tune = value,
            13 => self.params.tom_decay = value,
            _ => return,
        }
        self.update_param_cache();
    }

    fn is_drum(&self) -> bool {
        true
    }
}
