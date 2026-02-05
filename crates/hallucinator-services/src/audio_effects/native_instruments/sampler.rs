//! Sample-based instrument — loads a WAV file and plays it pitched across MIDI keyboard

use std::path::Path;
use std::sync::Arc;

use crate::audio_effects::{AudioInstrument, EffectParam};

const MAX_VOICES: usize = 8;
const MAX_BLOCK_SIZE: usize = 4096;

/// Per-voice state for the sampler
struct SamplerVoice {
    active: bool,
    pitch: u8,
    velocity: f32,
    position: f64,
    speed: f64,
    releasing: bool,
    release_gain: f32,
    release_step: f32,
    age: usize,
}

impl SamplerVoice {
    fn new() -> Self {
        Self {
            active: false,
            pitch: 0,
            velocity: 0.0,
            position: 0.0,
            speed: 1.0,
            releasing: false,
            release_gain: 1.0,
            release_step: 0.0,
            age: 0,
        }
    }

    fn trigger(&mut self, pitch: u8, velocity: u8, base_pitch: u8) {
        self.active = true;
        self.pitch = pitch;
        self.velocity = velocity as f32 / 127.0;
        self.position = 0.0;
        self.speed = 2.0_f64.powf((pitch as f64 - base_pitch as f64) / 12.0);
        self.releasing = false;
        self.release_gain = 1.0;
        self.release_step = 0.0;
        self.age = 0;
    }

    fn start_release(&mut self, sample_rate: f32) {
        if !self.releasing {
            self.releasing = true;
            let fade_samples = (sample_rate * 0.005).max(1.0); // 5ms
            self.release_step = 1.0 / fade_samples;
        }
    }

    fn tick(&mut self, sample_data: &[f32]) -> f32 {
        if !self.active {
            return 0.0;
        }

        self.age += 1;

        let pos = self.position;
        let idx = pos as usize;

        // End of sample
        if idx >= sample_data.len().saturating_sub(1) {
            self.active = false;
            return 0.0;
        }

        // Linear interpolation
        let frac = (pos - idx as f64) as f32;
        let s0 = sample_data[idx];
        let s1 = sample_data[idx + 1];
        let sample = s0 + frac * (s1 - s0);

        self.position += self.speed;

        // Release fade
        if self.releasing {
            self.release_gain -= self.release_step;
            if self.release_gain <= 0.0 {
                self.active = false;
                return 0.0;
            }
        }

        sample * self.velocity * self.release_gain
    }
}

/// Parameters for the sampler
#[derive(Debug, Clone)]
struct SamplerParams {
    master: f32,
}

impl Default for SamplerParams {
    fn default() -> Self {
        Self { master: 0.8 }
    }
}

/// Sample-based instrument
pub struct Sampler {
    sample_data: Arc<Vec<f32>>,
    sample_rate: f32,
    base_pitch: u8,
    voices: Vec<SamplerVoice>,
    pending_on: Vec<(u8, u8, u32)>,
    pending_off: Vec<(u8, u32)>,
    output_left: Vec<f32>,
    output_right: Vec<f32>,
    params: SamplerParams,
    param_cache: Vec<EffectParam>,
    inst_name: String,
}

impl std::fmt::Debug for Sampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sampler")
            .field("name", &self.inst_name)
            .field("sample_len", &self.sample_data.len())
            .field("base_pitch", &self.base_pitch)
            .finish()
    }
}

impl Sampler {
    /// Create a sampler from pre-loaded mono sample data (already at engine sample rate).
    pub fn new(name: String, sample_data: Vec<f32>, sample_rate: f32) -> Self {
        let voices = (0..MAX_VOICES).map(|_| SamplerVoice::new()).collect();
        let params = SamplerParams::default();
        let param_cache = Self::build_param_cache(&params);

        Self {
            sample_data: Arc::new(sample_data),
            sample_rate,
            base_pitch: 60, // C4
            voices,
            pending_on: Vec::new(),
            pending_off: Vec::new(),
            output_left: vec![0.0; MAX_BLOCK_SIZE],
            output_right: vec![0.0; MAX_BLOCK_SIZE],
            params,
            param_cache,
            inst_name: name,
        }
    }

    /// Load a sampler from a WAV file, resampling to the given engine sample rate.
    pub fn from_wav(path: &Path, engine_sample_rate: f32) -> Result<Self, String> {
        let reader = hound::WavReader::open(path)
            .map_err(|e| format!("Failed to open WAV: {e}"))?;

        let spec = reader.spec();
        let channels = spec.channels as usize;

        let raw_samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => {
                reader.into_samples::<f32>().filter_map(Result::ok).collect()
            }
            hound::SampleFormat::Int => {
                let max_val = (1_i64 << (spec.bits_per_sample - 1)) as f32;
                reader.into_samples::<i32>()
                    .filter_map(Result::ok)
                    .map(|s| s as f32 / max_val)
                    .collect()
            }
        };

        if raw_samples.is_empty() {
            return Err("WAV file is empty".into());
        }

        // Mix to mono if stereo
        let mono: Vec<f32> = if channels == 1 {
            raw_samples
        } else {
            raw_samples
                .chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect()
        };

        // Resample if needed
        let resampled = resample_linear(&mono, spec.sample_rate as f32, engine_sample_rate);

        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Sample")
            .to_string();

        Ok(Self::new(name, resampled, engine_sample_rate))
    }

    fn build_param_cache(params: &SamplerParams) -> Vec<EffectParam> {
        vec![
            EffectParam::new("master", params.master, 0.0, 1.0, ""),
        ]
    }

    fn update_param_cache(&mut self) {
        self.param_cache = Self::build_param_cache(&self.params);
    }

    fn find_voice_for_note_on(&mut self, pitch: u8) -> usize {
        // Prefer: same pitch (retrigger) → inactive → oldest
        self.voices.iter().position(|v| v.active && v.pitch == pitch)
            .or_else(|| self.voices.iter().position(|v| !v.active))
            .unwrap_or_else(|| {
                self.voices.iter()
                    .enumerate()
                    .max_by_key(|(_, v)| v.age)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            })
    }
}

impl AudioInstrument for Sampler {
    fn name(&self) -> &str {
        &self.inst_name
    }

    fn queue_note_on(&mut self, pitch: u8, velocity: u8, _channel: u8, sample_offset: u32) {
        self.pending_on.push((pitch, velocity, sample_offset));
    }

    fn queue_note_off(&mut self, pitch: u8, _velocity: u8, _channel: u8, sample_offset: u32) {
        self.pending_off.push((pitch, sample_offset));
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

        self.pending_on.sort_by_key(|e| e.2);
        self.pending_off.sort_by_key(|e| e.1);

        let sample_data = Arc::clone(&self.sample_data);

        for frame_idx in 0..frames {
            // Process note-on events at this frame
            while let Some(&(pitch, velocity, offset)) = self.pending_on.first() {
                if offset as usize > frame_idx { break; }
                self.pending_on.remove(0);
                let vi = self.find_voice_for_note_on(pitch);
                self.voices[vi].trigger(pitch, velocity, self.base_pitch);
            }

            // Process note-off events at this frame
            while let Some(&(pitch, offset)) = self.pending_off.first() {
                if offset as usize > frame_idx { break; }
                self.pending_off.remove(0);
                for voice in &mut self.voices {
                    if voice.active && voice.pitch == pitch && !voice.releasing {
                        voice.start_release(self.sample_rate);
                    }
                }
            }

            let mut mix = 0.0_f32;
            for voice in &mut self.voices {
                mix += voice.tick(&sample_data);
            }

            let out = (mix * self.params.master).clamp(-1.0, 1.0);
            self.output_left[frame_idx] = out;
            self.output_right[frame_idx] = out;
        }

        // Adjust remaining events
        self.pending_on.retain(|e| e.2 as usize >= frames);
        for event in &mut self.pending_on {
            event.2 -= frames as u32;
        }
        self.pending_off.retain(|e| e.1 as usize >= frames);
        for event in &mut self.pending_off {
            event.1 -= frames as u32;
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
        if name == "master" {
            self.params.master = value;
            self.update_param_cache();
        }
    }

    fn set_param_by_index(&mut self, index: usize, value: f64) {
        if index == 0 {
            self.params.master = value as f32;
            self.update_param_cache();
        }
    }
}

/// Simple linear resampling from source rate to target rate.
fn resample_linear(data: &[f32], from_rate: f32, to_rate: f32) -> Vec<f32> {
    if (from_rate - to_rate).abs() < 1.0 {
        return data.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (data.len() as f64 / ratio).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = (src_pos - idx as f64) as f32;

        let s0 = data.get(idx).copied().unwrap_or(0.0);
        let s1 = data.get(idx + 1).copied().unwrap_or(s0);
        out.push(s0 + frac * (s1 - s0));
    }

    out
}
