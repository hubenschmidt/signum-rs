//! MIDI effects for Hapax-style real-time processing

use serde::{Deserialize, Serialize};

/// A MIDI event for FX processing
#[derive(Debug, Clone, Copy)]
pub struct MidiEvent {
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
    pub sample_offset: u32,
    pub is_note_on: bool,
}

/// Parameter for MIDI effects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiFxParam {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

impl MidiFxParam {
    pub fn new(name: &str, value: f32, min: f32, max: f32) -> Self {
        Self { name: name.to_string(), value, min, max }
    }
}

/// Trait for MIDI effects
pub trait MidiFx: Send {
    fn name(&self) -> &str;
    fn process(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent>;
    fn get_params(&self) -> &[MidiFxParam];
    fn set_param(&mut self, name: &str, value: f32);
    fn is_bypassed(&self) -> bool;
    fn set_bypass(&mut self, bypass: bool);
}

/// Enum wrapper for all MIDI effects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MidiEffect {
    Transpose(TransposeFx),
    Quantize(QuantizeFx),
    Swing(SwingFx),
    Humanize(HumanizeFx),
    Chance(ChanceFx),
    Echo(EchoFx),
    Arpeggiator(ArpeggiatorFx),
    Harmonizer(HarmonizerFx),
}

impl MidiEffect {
    pub fn name(&self) -> &str {
        match self {
            Self::Transpose(fx) => fx.name(),
            Self::Quantize(fx) => fx.name(),
            Self::Swing(fx) => fx.name(),
            Self::Humanize(fx) => fx.name(),
            Self::Chance(fx) => fx.name(),
            Self::Echo(fx) => fx.name(),
            Self::Arpeggiator(fx) => fx.name(),
            Self::Harmonizer(fx) => fx.name(),
        }
    }

    pub fn process(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        match self {
            Self::Transpose(fx) => fx.process(events, sample_rate, bpm),
            Self::Quantize(fx) => fx.process(events, sample_rate, bpm),
            Self::Swing(fx) => fx.process(events, sample_rate, bpm),
            Self::Humanize(fx) => fx.process(events, sample_rate, bpm),
            Self::Chance(fx) => fx.process(events, sample_rate, bpm),
            Self::Echo(fx) => fx.process(events, sample_rate, bpm),
            Self::Arpeggiator(fx) => fx.process(events, sample_rate, bpm),
            Self::Harmonizer(fx) => fx.process(events, sample_rate, bpm),
        }
    }

    pub fn get_params(&self) -> &[MidiFxParam] {
        match self {
            Self::Transpose(fx) => fx.get_params(),
            Self::Quantize(fx) => fx.get_params(),
            Self::Swing(fx) => fx.get_params(),
            Self::Humanize(fx) => fx.get_params(),
            Self::Chance(fx) => fx.get_params(),
            Self::Echo(fx) => fx.get_params(),
            Self::Arpeggiator(fx) => fx.get_params(),
            Self::Harmonizer(fx) => fx.get_params(),
        }
    }

    pub fn set_param(&mut self, name: &str, value: f32) {
        match self {
            Self::Transpose(fx) => fx.set_param(name, value),
            Self::Quantize(fx) => fx.set_param(name, value),
            Self::Swing(fx) => fx.set_param(name, value),
            Self::Humanize(fx) => fx.set_param(name, value),
            Self::Chance(fx) => fx.set_param(name, value),
            Self::Echo(fx) => fx.set_param(name, value),
            Self::Arpeggiator(fx) => fx.set_param(name, value),
            Self::Harmonizer(fx) => fx.set_param(name, value),
        }
    }

    pub fn is_bypassed(&self) -> bool {
        match self {
            Self::Transpose(fx) => fx.is_bypassed(),
            Self::Quantize(fx) => fx.is_bypassed(),
            Self::Swing(fx) => fx.is_bypassed(),
            Self::Humanize(fx) => fx.is_bypassed(),
            Self::Chance(fx) => fx.is_bypassed(),
            Self::Echo(fx) => fx.is_bypassed(),
            Self::Arpeggiator(fx) => fx.is_bypassed(),
            Self::Harmonizer(fx) => fx.is_bypassed(),
        }
    }

    pub fn set_bypass(&mut self, bypass: bool) {
        match self {
            Self::Transpose(fx) => fx.set_bypass(bypass),
            Self::Quantize(fx) => fx.set_bypass(bypass),
            Self::Swing(fx) => fx.set_bypass(bypass),
            Self::Humanize(fx) => fx.set_bypass(bypass),
            Self::Chance(fx) => fx.set_bypass(bypass),
            Self::Echo(fx) => fx.set_bypass(bypass),
            Self::Arpeggiator(fx) => fx.set_bypass(bypass),
            Self::Harmonizer(fx) => fx.set_bypass(bypass),
        }
    }
}

// ============================================================================
// Transpose Effect
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransposeFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
}

impl Default for TransposeFx {
    fn default() -> Self {
        Self {
            params: vec![MidiFxParam::new("semitones", 0.0, -48.0, 48.0)],
            bypass: false,
        }
    }
}

impl MidiFx for TransposeFx {
    fn name(&self) -> &str { "Transpose" }

    fn process(&mut self, events: Vec<MidiEvent>, _sample_rate: f32, _bpm: f64) -> Vec<MidiEvent> {
        if self.bypass { return events; }
        let semitones = self.params[0].value as i32;
        events.into_iter().map(|mut e| {
            let new_pitch = (e.pitch as i32 + semitones).clamp(0, 127) as u8;
            e.pitch = new_pitch;
            e
        }).collect()
    }

    fn get_params(&self) -> &[MidiFxParam] { &self.params }

    fn set_param(&mut self, name: &str, value: f32) {
        if let Some(p) = self.params.iter_mut().find(|p| p.name == name) {
            p.value = value.clamp(p.min, p.max);
        }
    }

    fn is_bypassed(&self) -> bool { self.bypass }
    fn set_bypass(&mut self, bypass: bool) { self.bypass = bypass; }
}

// ============================================================================
// Quantize Effect
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizeFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
}

impl Default for QuantizeFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("grid", 4.0, 1.0, 32.0),      // 1=whole, 4=quarter, 8=eighth, etc.
                MidiFxParam::new("strength", 100.0, 0.0, 100.0),
            ],
            bypass: false,
        }
    }
}

impl MidiFx for QuantizeFx {
    fn name(&self) -> &str { "Quantize" }

    fn process(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        if self.bypass { return events; }
        let grid = self.params[0].value;
        let strength = self.params[1].value / 100.0;

        // Samples per beat at current BPM
        let samples_per_beat = (sample_rate as f64 * 60.0 / bpm) as u32;
        // Grid division in samples
        let grid_samples = samples_per_beat * 4 / grid as u32;

        events.into_iter().map(|mut e| {
            let nearest_grid = ((e.sample_offset as f64 / grid_samples as f64).round() * grid_samples as f64) as u32;
            let diff = nearest_grid as i32 - e.sample_offset as i32;
            e.sample_offset = (e.sample_offset as i32 + (diff as f32 * strength) as i32).max(0) as u32;
            e
        }).collect()
    }

    fn get_params(&self) -> &[MidiFxParam] { &self.params }

    fn set_param(&mut self, name: &str, value: f32) {
        if let Some(p) = self.params.iter_mut().find(|p| p.name == name) {
            p.value = value.clamp(p.min, p.max);
        }
    }

    fn is_bypassed(&self) -> bool { self.bypass }
    fn set_bypass(&mut self, bypass: bool) { self.bypass = bypass; }
}

// ============================================================================
// Swing Effect
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwingFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
}

impl Default for SwingFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("amount", 50.0, 0.0, 100.0),  // 50% = no swing
                MidiFxParam::new("grid", 8.0, 4.0, 16.0),      // Apply swing to 8th notes by default
            ],
            bypass: false,
        }
    }
}

impl MidiFx for SwingFx {
    fn name(&self) -> &str { "Swing" }

    fn process(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        if self.bypass { return events; }
        let amount = self.params[0].value / 100.0;
        let grid = self.params[1].value;

        let samples_per_beat = (sample_rate as f64 * 60.0 / bpm) as u32;
        let grid_samples = samples_per_beat * 4 / grid as u32;

        // Swing ratio: 50% = straight, 66% = triplet feel
        let swing_ratio = 0.5 + (amount - 0.5) * 0.33;

        events.into_iter().map(|mut e| {
            let grid_pos = e.sample_offset / grid_samples;
            let _pos_in_grid = e.sample_offset % grid_samples;

            // Only swing off-beats (odd grid positions)
            if grid_pos % 2 == 1 {
                let swing_offset = ((swing_ratio as f64 - 0.5) * grid_samples as f64) as i32;
                e.sample_offset = (e.sample_offset as i32 + swing_offset).max(0) as u32;
            }
            e
        }).collect()
    }

    fn get_params(&self) -> &[MidiFxParam] { &self.params }

    fn set_param(&mut self, name: &str, value: f32) {
        if let Some(p) = self.params.iter_mut().find(|p| p.name == name) {
            p.value = value.clamp(p.min, p.max);
        }
    }

    fn is_bypassed(&self) -> bool { self.bypass }
    fn set_bypass(&mut self, bypass: bool) { self.bypass = bypass; }
}

// ============================================================================
// Humanize Effect
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanizeFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
    rng_state: u64,
}

impl Default for HumanizeFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("timing", 10.0, 0.0, 50.0),    // ms variance
                MidiFxParam::new("velocity", 10.0, 0.0, 30.0),  // velocity variance
            ],
            bypass: false,
            rng_state: 12345,
        }
    }
}

impl HumanizeFx {
    fn next_random(&mut self) -> f32 {
        // Simple LCG for deterministic randomness
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((self.rng_state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl MidiFx for HumanizeFx {
    fn name(&self) -> &str { "Humanize" }

    fn process(&mut self, events: Vec<MidiEvent>, sample_rate: f32, _bpm: f64) -> Vec<MidiEvent> {
        if self.bypass { return events; }
        let timing_ms = self.params[0].value;
        let vel_var = self.params[1].value;

        let timing_samples = (timing_ms / 1000.0 * sample_rate) as i32;

        events.into_iter().map(|mut e| {
            let time_offset = (self.next_random() * timing_samples as f32) as i32;
            let vel_offset = (self.next_random() * vel_var) as i8;

            e.sample_offset = (e.sample_offset as i32 + time_offset).max(0) as u32;
            e.velocity = (e.velocity as i16 + vel_offset as i16).clamp(1, 127) as u8;
            e
        }).collect()
    }

    fn get_params(&self) -> &[MidiFxParam] { &self.params }

    fn set_param(&mut self, name: &str, value: f32) {
        if let Some(p) = self.params.iter_mut().find(|p| p.name == name) {
            p.value = value.clamp(p.min, p.max);
        }
    }

    fn is_bypassed(&self) -> bool { self.bypass }
    fn set_bypass(&mut self, bypass: bool) { self.bypass = bypass; }
}

// ============================================================================
// Chance Effect
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChanceFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
    rng_state: u64,
}

impl Default for ChanceFx {
    fn default() -> Self {
        Self {
            params: vec![MidiFxParam::new("probability", 100.0, 0.0, 100.0)],
            bypass: false,
            rng_state: 54321,
        }
    }
}

impl ChanceFx {
    fn next_random(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.rng_state >> 33) as f32 / u32::MAX as f32
    }
}

impl MidiFx for ChanceFx {
    fn name(&self) -> &str { "Chance" }

    fn process(&mut self, events: Vec<MidiEvent>, _sample_rate: f32, _bpm: f64) -> Vec<MidiEvent> {
        if self.bypass { return events; }
        let prob = self.params[0].value / 100.0;

        events.into_iter().filter(|_| self.next_random() < prob).collect()
    }

    fn get_params(&self) -> &[MidiFxParam] { &self.params }

    fn set_param(&mut self, name: &str, value: f32) {
        if let Some(p) = self.params.iter_mut().find(|p| p.name == name) {
            p.value = value.clamp(p.min, p.max);
        }
    }

    fn is_bypassed(&self) -> bool { self.bypass }
    fn set_bypass(&mut self, bypass: bool) { self.bypass = bypass; }
}

// ============================================================================
// Echo Effect (MIDI delay/repeat)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EchoFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
}

impl Default for EchoFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("delay", 4.0, 1.0, 16.0),      // In grid divisions (4 = quarter note)
                MidiFxParam::new("feedback", 3.0, 1.0, 8.0),    // Number of repeats
                MidiFxParam::new("decay", 70.0, 0.0, 100.0),    // Velocity decay per repeat
            ],
            bypass: false,
        }
    }
}

impl MidiFx for EchoFx {
    fn name(&self) -> &str { "Echo" }

    fn process(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        if self.bypass { return events; }
        let delay_div = self.params[0].value;
        let repeats = self.params[1].value as u32;
        let decay = self.params[2].value / 100.0;

        let samples_per_beat = (sample_rate as f64 * 60.0 / bpm) as u32;
        let delay_samples = samples_per_beat * 4 / delay_div as u32;

        let mut result = events.clone();

        for event in &events {
            if !event.is_note_on { continue; }

            let mut vel = event.velocity as f32;
            for i in 1..=repeats {
                vel *= decay;
                if vel < 1.0 { break; }

                result.push(MidiEvent {
                    pitch: event.pitch,
                    velocity: vel as u8,
                    channel: event.channel,
                    sample_offset: event.sample_offset + delay_samples * i,
                    is_note_on: true,
                });
            }
        }

        result.sort_by_key(|e| e.sample_offset);
        result
    }

    fn get_params(&self) -> &[MidiFxParam] { &self.params }

    fn set_param(&mut self, name: &str, value: f32) {
        if let Some(p) = self.params.iter_mut().find(|p| p.name == name) {
            p.value = value.clamp(p.min, p.max);
        }
    }

    fn is_bypassed(&self) -> bool { self.bypass }
    fn set_bypass(&mut self, bypass: bool) { self.bypass = bypass; }
}

// ============================================================================
// Arpeggiator Effect
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArpMode {
    Up,
    Down,
    UpDown,
    Random,
    Order,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArpeggiatorFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
    held_notes: Vec<u8>,
    rng_state: u64,
}

impl Default for ArpeggiatorFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("mode", 0.0, 0.0, 4.0),        // 0=Up, 1=Down, 2=UpDown, 3=Random, 4=Order
                MidiFxParam::new("rate", 8.0, 1.0, 32.0),       // Notes per beat (8 = 8th notes)
                MidiFxParam::new("octaves", 1.0, 1.0, 4.0),     // Octave range
                MidiFxParam::new("gate", 80.0, 10.0, 100.0),    // Gate length %
            ],
            bypass: false,
            held_notes: Vec::new(),
            rng_state: 99999,
        }
    }
}

impl ArpeggiatorFx {
    fn get_mode(&self) -> ArpMode {
        match self.params[0].value as u8 {
            0 => ArpMode::Up,
            1 => ArpMode::Down,
            2 => ArpMode::UpDown,
            3 => ArpMode::Random,
            _ => ArpMode::Order,
        }
    }

    fn next_random(&mut self) -> usize {
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.rng_state >> 33) as usize
    }
}

impl MidiFx for ArpeggiatorFx {
    fn name(&self) -> &str { "Arpeggiator" }

    fn process(&mut self, events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        if self.bypass { return events; }

        // Update held notes
        for event in &events {
            if event.is_note_on {
                if !self.held_notes.contains(&event.pitch) {
                    self.held_notes.push(event.pitch);
                }
            } else {
                self.held_notes.retain(|&p| p != event.pitch);
            }
        }

        if self.held_notes.is_empty() { return vec![]; }

        let rate = self.params[1].value;
        let octaves = self.params[2].value as u8;
        let gate = self.params[3].value / 100.0;

        let samples_per_beat = (sample_rate as f64 * 60.0 / bpm) as u32;
        let note_samples = samples_per_beat * 4 / rate as u32;
        let gate_samples = (note_samples as f32 * gate) as u32;

        // Build arp sequence
        let mut sequence: Vec<u8> = Vec::new();
        let mode = self.get_mode();

        let mut sorted_notes = self.held_notes.clone();
        sorted_notes.sort();

        for oct in 0..octaves {
            let oct_offset = oct * 12;
            match mode {
                ArpMode::Up | ArpMode::UpDown | ArpMode::Order => {
                    for &note in &sorted_notes {
                        let new_note = (note as u16 + oct_offset as u16).min(127) as u8;
                        sequence.push(new_note);
                    }
                }
                ArpMode::Down => {
                    for &note in sorted_notes.iter().rev() {
                        let new_note = (note as u16 + oct_offset as u16).min(127) as u8;
                        sequence.push(new_note);
                    }
                }
                ArpMode::Random => {
                    for &note in &sorted_notes {
                        let new_note = (note as u16 + oct_offset as u16).min(127) as u8;
                        sequence.push(new_note);
                    }
                }
            }
        }

        if mode == ArpMode::UpDown && sequence.len() > 2 {
            let down: Vec<u8> = sequence[1..sequence.len()-1].iter().rev().copied().collect();
            sequence.extend(down);
        }

        if mode == ArpMode::Random && !sequence.is_empty() {
            // Fisher-Yates shuffle
            for i in (1..sequence.len()).rev() {
                let j = self.next_random() % (i + 1);
                sequence.swap(i, j);
            }
        }

        // Generate arp notes
        let mut result = Vec::new();
        let channel = events.first().map(|e| e.channel).unwrap_or(0);

        for (i, &pitch) in sequence.iter().enumerate() {
            let offset = note_samples * i as u32;
            result.push(MidiEvent {
                pitch,
                velocity: 100,
                channel,
                sample_offset: offset,
                is_note_on: true,
            });
            result.push(MidiEvent {
                pitch,
                velocity: 0,
                channel,
                sample_offset: offset + gate_samples,
                is_note_on: false,
            });
        }

        result
    }

    fn get_params(&self) -> &[MidiFxParam] { &self.params }

    fn set_param(&mut self, name: &str, value: f32) {
        if let Some(p) = self.params.iter_mut().find(|p| p.name == name) {
            p.value = value.clamp(p.min, p.max);
        }
    }

    fn is_bypassed(&self) -> bool { self.bypass }
    fn set_bypass(&mut self, bypass: bool) { self.bypass = bypass; }
}

// ============================================================================
// Harmonizer Effect
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmonizerFx {
    params: Vec<MidiFxParam>,
    bypass: bool,
}

impl Default for HarmonizerFx {
    fn default() -> Self {
        Self {
            params: vec![
                MidiFxParam::new("interval1", 4.0, -12.0, 12.0),  // Major third
                MidiFxParam::new("interval2", 7.0, -12.0, 12.0),  // Perfect fifth
                MidiFxParam::new("voices", 2.0, 0.0, 2.0),        // How many harmony voices
            ],
            bypass: false,
        }
    }
}

impl MidiFx for HarmonizerFx {
    fn name(&self) -> &str { "Harmonizer" }

    fn process(&mut self, events: Vec<MidiEvent>, _sample_rate: f32, _bpm: f64) -> Vec<MidiEvent> {
        if self.bypass { return events; }

        let interval1 = self.params[0].value as i8;
        let interval2 = self.params[1].value as i8;
        let voices = self.params[2].value as u8;

        let mut result = events.clone();

        for event in &events {
            if !event.is_note_on { continue; }

            if voices >= 1 {
                let pitch1 = (event.pitch as i16 + interval1 as i16).clamp(0, 127) as u8;
                result.push(MidiEvent { pitch: pitch1, ..*event });
            }
            if voices >= 2 {
                let pitch2 = (event.pitch as i16 + interval2 as i16).clamp(0, 127) as u8;
                result.push(MidiEvent { pitch: pitch2, ..*event });
            }
        }

        result
    }

    fn get_params(&self) -> &[MidiFxParam] { &self.params }

    fn set_param(&mut self, name: &str, value: f32) {
        if let Some(p) = self.params.iter_mut().find(|p| p.name == name) {
            p.value = value.clamp(p.min, p.max);
        }
    }

    fn is_bypassed(&self) -> bool { self.bypass }
    fn set_bypass(&mut self, bypass: bool) { self.bypass = bypass; }
}

// ============================================================================
// MIDI FX Chain
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MidiFxChain {
    pub effects: Vec<MidiEffect>,
    pub bypass_all: bool,
}

impl MidiFxChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, effect: MidiEffect) {
        if self.effects.len() < 8 {
            self.effects.push(effect);
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<MidiEffect> {
        if index < self.effects.len() {
            return Some(self.effects.remove(index));
        }
        None
    }

    pub fn process(&mut self, mut events: Vec<MidiEvent>, sample_rate: f32, bpm: f64) -> Vec<MidiEvent> {
        if self.bypass_all { return events; }

        for effect in &mut self.effects {
            if !effect.is_bypassed() {
                events = effect.process(events, sample_rate, bpm);
            }
        }
        events
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}
