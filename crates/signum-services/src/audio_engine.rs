//! Audio engine for timeline playback

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use signum_core::{MidiClip, MidiEvent, Timeline, TrackKind};
use thiserror::Error;
use tracing::info;

use crate::audio_io::{AudioOutputError, RealtimeOutputStream};
use crate::audio_effects::{EffectChain, Instrument};

#[derive(Debug, Error)]
pub enum AudioEngineError {
    #[error("Audio output error: {0}")]
    Output(#[from] AudioOutputError),
    #[error("Engine already running")]
    AlreadyRunning,
    #[error("Engine not running")]
    NotRunning,
}

/// Audio engine state shared between UI and audio thread
pub struct EngineState {
    /// Current playback position in samples
    pub position: AtomicU64,
    /// Playing flag
    pub playing: AtomicBool,
    /// Timeline data (protected by mutex for clip access)
    pub timeline: Mutex<Timeline>,
    /// Master effect chain
    pub master_effects: Mutex<EffectChain>,
    /// Instruments (VST3 or native) keyed by instrument ID
    pub instruments: Mutex<HashMap<u64, Instrument>>,
    /// Per-track effect chains keyed by chain ID
    pub track_effects: Mutex<HashMap<u64, EffectChain>>,
}

impl EngineState {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            position: AtomicU64::new(0),
            playing: AtomicBool::new(false),
            timeline: Mutex::new(Timeline::new(sample_rate)),
            master_effects: Mutex::new(EffectChain::new()),
            instruments: Mutex::new(HashMap::new()),
            track_effects: Mutex::new(HashMap::new()),
        }
    }
}

/// Audio engine for DAW playback
pub struct AudioEngine {
    state: Arc<EngineState>,
    stream: Option<RealtimeOutputStream>,
    sample_rate: u32,
}

impl AudioEngine {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            state: Arc::new(EngineState::new(sample_rate)),
            stream: None,
            sample_rate,
        }
    }

    /// Get shared state for UI access
    pub fn state(&self) -> Arc<EngineState> {
        self.state.clone()
    }

    /// Start the audio engine
    pub fn start(&mut self) -> Result<(), AudioEngineError> {
        if self.stream.is_some() {
            return Err(AudioEngineError::AlreadyRunning);
        }

        let state = self.state.clone();

        let stream = RealtimeOutputStream::start(move |buffer, _sample_rate, channels| {
            Self::render_audio(&state, buffer, channels);
        })?;

        self.stream = Some(stream);
        info!("Audio engine started");
        Ok(())
    }

    /// Stop the audio engine
    pub fn stop(&mut self) -> Result<(), AudioEngineError> {
        let stream = self.stream.take().ok_or(AudioEngineError::NotRunning)?;
        stream.stop();
        self.state.playing.store(false, Ordering::SeqCst);
        info!("Audio engine stopped");
        Ok(())
    }

    /// Play from current position
    pub fn play(&self) {
        self.state.playing.store(true, Ordering::SeqCst);
        if let Ok(mut timeline) = self.state.timeline.lock() {
            timeline.transport.play();
        }
    }

    /// Pause playback
    pub fn pause(&self) {
        self.state.playing.store(false, Ordering::SeqCst);
        if let Ok(mut timeline) = self.state.timeline.lock() {
            timeline.transport.pause();
        }
    }

    /// Stop and reset to beginning
    pub fn stop_playback(&self) {
        self.state.playing.store(false, Ordering::SeqCst);
        self.state.position.store(0, Ordering::SeqCst);
        if let Ok(mut timeline) = self.state.timeline.lock() {
            timeline.transport.stop();
        }
    }

    /// Seek to position in samples
    pub fn seek(&self, position_samples: u64) {
        self.state.position.store(position_samples, Ordering::SeqCst);
        if let Ok(mut timeline) = self.state.timeline.lock() {
            timeline.transport.position_samples = position_samples;
        }
    }

    /// Set loop region (start and end in samples)
    pub fn set_loop_region(&self, start_samples: u64, end_samples: u64) {
        if let Ok(mut timeline) = self.state.timeline.lock() {
            timeline.transport.loop_start = start_samples;
            timeline.transport.loop_end = end_samples;
        }
    }

    /// Enable or disable loop mode
    pub fn set_loop_enabled(&self, enabled: bool) {
        if let Ok(mut timeline) = self.state.timeline.lock() {
            timeline.transport.loop_enabled = enabled;
        }
    }

    /// Check if loop mode is enabled
    pub fn is_loop_enabled(&self) -> bool {
        self.state.timeline.lock()
            .map(|t| t.transport.loop_enabled)
            .unwrap_or(false)
    }

    /// Get loop region (start, end) in samples
    pub fn loop_region(&self) -> (u64, u64) {
        self.state.timeline.lock()
            .map(|t| (t.transport.loop_start, t.transport.loop_end))
            .unwrap_or((0, 0))
    }

    /// Get current position in samples
    pub fn position(&self) -> u64 {
        self.state.position.load(Ordering::SeqCst)
    }

    /// Check if playing
    pub fn is_playing(&self) -> bool {
        self.state.playing.load(Ordering::SeqCst)
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Render audio into output buffer (called from audio thread)
    fn render_audio(state: &EngineState, buffer: &mut [f32], channels: u16) {
        let is_playing = state.playing.load(Ordering::SeqCst);
        let mut pos = state.position.load(Ordering::SeqCst);
        let channels = channels as usize;
        let num_frames = buffer.len() / channels;

        // Lock instruments once for both preview and playback processing
        let Ok(mut instruments) = state.instruments.lock() else {
            buffer.fill(0.0);
            return;
        };

        // If playing, also lock timeline and queue MIDI events from clips
        let timeline_data = if is_playing {
            let Ok(mut timeline) = state.timeline.lock() else {
                drop(instruments);
                buffer.fill(0.0);
                return;
            };

            let duration = timeline.duration_samples();
            let loop_enabled = timeline.transport.loop_enabled;
            let loop_start = timeline.transport.loop_start;
            let loop_end = timeline.transport.loop_end;
            let bpm = timeline.transport.bpm;
            let sample_rate = timeline.transport.sample_rate;

            // Queue MIDI events for each instrument from clips
            // Handle loop wrap: if buffer spans loop_end, collect from both regions
            for track in timeline.tracks.iter_mut().filter(|t| t.kind == TrackKind::Midi && !t.mute) {
                let Some(inst_id) = track.instrument_id else {
                    tracing::trace!("MIDI track '{}' has no instrument", track.name);
                    continue;
                };
                let Some(instrument) = instruments.get_mut(&inst_id) else {
                    tracing::warn!("Instrument {} not found for track '{}'", inst_id, track.name);
                    continue;
                };

                let buffer_end = pos + num_frames as u64;
                let spans_loop = loop_enabled && loop_end > loop_start && pos < loop_end && buffer_end > loop_end;

                // Send note off for active notes at loop boundary to stop hanging notes
                // Skip for drum instruments - they're one-shot and should decay naturally
                if spans_loop && !instrument.is_drum() {
                    let frames_before_loop = (loop_end - pos) as usize;
                    instrument.all_notes_off(frames_before_loop as u32);
                }

                // Collect raw MIDI events from all clips
                let mut raw_events: Vec<MidiEvent> = Vec::new();

                for clip in &track.midi_clips {
                    tracing::trace!("MIDI collect: pos={} buffer_end={} loop={}..{} spans={}", pos, buffer_end, loop_start, loop_end, spans_loop);

                    if spans_loop {
                        // Part 1: from pos to loop_end
                        let frames_before_loop = (loop_end - pos) as usize;
                        Self::collect_midi_events_raw(clip, pos, frames_before_loop, bpm, sample_rate, &mut raw_events, 0);

                        // Part 2: from loop_start for remaining frames
                        let frames_after_loop = num_frames - frames_before_loop;
                        Self::collect_midi_events_raw(clip, loop_start, frames_after_loop, bpm, sample_rate, &mut raw_events, frames_before_loop as u32);
                    } else {
                        Self::collect_midi_events_raw(clip, pos, num_frames, bpm, sample_rate, &mut raw_events, 0);
                    }
                }

                // Process through MIDI FX chain
                let processed_events = track.midi_fx_chain.process(raw_events, sample_rate as f32, bpm);

                // Queue processed events to instrument
                for event in processed_events {
                    if event.is_note_on {
                        instrument.queue_note_on(event.pitch, event.velocity, event.channel, event.sample_offset.max(1));
                    } else {
                        instrument.queue_note_off(event.pitch, event.velocity, event.channel, event.sample_offset);
                    }
                }
            }

            Some((duration, loop_enabled, loop_start, loop_end, timeline))
        } else {
            None
        };

        // Process instruments and collect their output (always, for keyboard preview)
        let mut instrument_mix = vec![0.0f32; num_frames];
        for instrument in instruments.values_mut() {
            let (left, right) = instrument.process(num_frames);
            for (i, sample) in instrument_mix.iter_mut().enumerate() {
                let l = left.get(i).copied().unwrap_or(0.0);
                let r = right.get(i).copied().unwrap_or(0.0);
                *sample += (l + r) * 0.5; // Mix stereo to mono
            }
        }

        drop(instruments);

        // Fill buffer based on playback state
        if let Some((duration, loop_enabled, loop_start, loop_end, timeline)) = timeline_data {
            // Playing: mix audio tracks with instruments
            let mut frame_idx = 0;
            for frame in buffer.chunks_mut(channels) {
                if loop_enabled && loop_end > loop_start && pos >= loop_end {
                    pos = loop_start;
                }

                let at_end = !loop_enabled && duration > 0 && pos >= duration;
                let audio_sample = if at_end { 0.0 } else { timeline.sample_at(pos) };
                let midi_sample = instrument_mix.get(frame_idx).copied().unwrap_or(0.0);
                frame.fill(audio_sample + midi_sample);
                pos += 1;
                frame_idx += 1;

                if at_end {
                    state.playing.store(false, Ordering::SeqCst);
                }
            }

            drop(timeline);
            state.position.store(pos, Ordering::SeqCst);
        } else {
            // Not playing: just output instrument mix (for keyboard preview)
            for (i, frame) in buffer.chunks_mut(channels).enumerate() {
                let sample = instrument_mix.get(i).copied().unwrap_or(0.0);
                frame.fill(sample);
            }
        }

        // Apply master effects
        if let Ok(mut effects) = state.master_effects.lock() {
            let mut mono: Vec<f32> = buffer
                .chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect();

            effects.process(&mut mono);

            for (i, frame) in buffer.chunks_mut(channels).enumerate() {
                let sample = mono.get(i).copied().unwrap_or(0.0);
                frame.fill(sample);
            }
        }
    }

    /// Collect MIDI events from a clip into a Vec (for MIDI FX processing)
    fn collect_midi_events_raw(
        clip: &MidiClip,
        buffer_start: u64,
        buffer_frames: usize,
        bpm: f64,
        sample_rate: u32,
        events: &mut Vec<MidiEvent>,
        base_offset: u32,
    ) {
        let buffer_end = buffer_start + buffer_frames as u64;

        // Clip bounds check
        if buffer_end <= clip.start_sample || buffer_start >= clip.end_sample() {
            return;
        }

        let samples_per_tick = (sample_rate as f64 * 60.0) / (bpm * clip.ppq as f64);

        for note in &clip.notes {
            let note_start_sample = clip.start_sample + (note.start_tick as f64 * samples_per_tick) as u64;
            let note_end_sample = note_start_sample + (note.duration_ticks as f64 * samples_per_tick) as u64;

            // Note On in this buffer?
            if note_start_sample >= buffer_start && note_start_sample < buffer_end {
                let offset = base_offset + (note_start_sample - buffer_start) as u32;
                events.push(MidiEvent {
                    pitch: note.pitch,
                    velocity: note.velocity,
                    channel: 0,
                    sample_offset: offset,
                    is_note_on: true,
                });
            }

            // Note Off in this buffer?
            if note_end_sample >= buffer_start && note_end_sample < buffer_end {
                let offset = base_offset + (note_end_sample - buffer_start) as u32;
                events.push(MidiEvent {
                    pitch: note.pitch,
                    velocity: 64,
                    channel: 0,
                    sample_offset: offset,
                    is_note_on: false,
                });
            }
        }
    }

    /// Access timeline for modification (use sparingly, locks mutex)
    pub fn with_timeline<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut Timeline) -> R,
    {
        self.state.timeline.lock().ok().map(|mut t| f(&mut t))
    }

    /// Access master effects chain
    pub fn with_master_effects<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut EffectChain) -> R,
    {
        self.state.master_effects.lock().ok().map(|mut e| f(&mut e))
    }

    /// Add an instrument with the given ID
    pub fn add_instrument(&self, id: u64, instrument: Instrument) {
        if let Ok(mut instruments) = self.state.instruments.lock() {
            instruments.insert(id, instrument);
        }
    }

    /// Remove an instrument by ID
    pub fn remove_instrument(&self, id: u64) -> Option<Instrument> {
        self.state.instruments.lock().ok()?.remove(&id)
    }

    /// Access instruments for modification
    pub fn with_instruments<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut HashMap<u64, Instrument>) -> R,
    {
        self.state.instruments.lock().ok().map(|mut i| f(&mut i))
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
