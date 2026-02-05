//! Main application state

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::CreationContext;
use egui::{Context, Vec2};
use signum_core::{AudioClip, ClipId, MidiClip, PlaybackMode, SongSection, TrackKind};
use signum_services::{
    AudioEngine, Drum808, EngineState, InputMonitor, Instrument, MeterState, PluginGuiManager,
    SampleKit, Sampler, Vst3Effect, Vst3Instrument, Vst3PluginInfo,
};

use crate::clipboard::{ClipboardContent, DawClipboard};
use crate::panels::{
    ArrangeAction, ArrangePanel, BrowserAction, BrowserPanel, ClipEditorPanel,
    DeviceInfo, DeviceRackAction, DeviceRackPanel, DrumRollAction, DrumRollPanel,
    KeyboardSequencerAction, KeyboardSequencerPanel,
    MidiEffectType, MidiFxRackAction, MidiFxRackPanel,
    PatternBankAction, PatternBankPanel,
    PianoRollAction, PluginAction, PluginBrowserPanel,
    RecordingPreview, SongViewAction, SongViewPanel,
    TrackHeaderAction, TrackHeadersPanel, TransportAction, TransportPanel,
};

// ── App config persistence ──────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct AppConfig {
    #[serde(default)]
    library: LibraryConfig,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct LibraryConfig {
    #[serde(default)]
    places: Vec<String>,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("signum")
        .join("config.toml")
}

fn load_config() -> AppConfig {
    let path = config_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(config: &AppConfig) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(s) = toml::to_string_pretty(config) else { return };
    let _ = std::fs::write(&path, s);
}

/// Floating plugin window state
struct PluginWindow {
    id: u64,
    title: String,
    plugin_path: String,
    plugin_uid: String,
    open: bool,
    native_window_created: bool,
}

/// Selected clip info
#[derive(Clone, Copy)]
pub enum SelectedClip {
    Audio { track_idx: usize, clip_id: ClipId },
    Midi { track_idx: usize, clip_id: ClipId },
}

pub struct SignumApp {
    engine: AudioEngine,
    engine_state: Arc<EngineState>,
    input_monitor: InputMonitor,
    meter_state: Arc<MeterState>,

    // Panels
    transport_panel: TransportPanel,
    plugin_menu: PluginBrowserPanel,
    browser_panel: BrowserPanel,
    track_headers_panel: TrackHeadersPanel,
    arrange_panel: ArrangePanel,
    device_rack_panel: DeviceRackPanel,
    clip_editor_panel: ClipEditorPanel,
    drum_roll_panel: DrumRollPanel,
    pattern_bank_panel: PatternBankPanel,
    keyboard_sequencer_panel: KeyboardSequencerPanel,
    midi_fx_rack_panel: MidiFxRackPanel,
    song_view_panel: SongViewPanel,

    // App-wide clipboard
    clipboard: DawClipboard,

    // Hapax panel visibility
    show_hapax_panels: bool,

    // Selection state
    selected_track_idx: Option<usize>,
    selected_clip: Option<SelectedClip>,
    show_clip_editor: bool,

    // Floating windows
    plugin_windows: Vec<PluginWindow>,
    native_param_windows: HashSet<u64>,  // IDs of native instruments with open param windows
    gui_manager: PluginGuiManager,

    // ID counters
    next_clip_id: u64,
    next_instrument_id: u64,
    next_effect_chain_id: u64,

    // Recording state
    recording_start_sample: u64,

    // Playback start position (for space toggle return-to-start)
    playback_start_position: u64,
}

fn to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 { return samples.to_vec(); }
    samples.chunks(channels)
        .map(|ch| ch.iter().sum::<f32>() / channels as f32)
        .collect()
}

impl SignumApp {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        let sample_rate = 44100;
        let mut engine = AudioEngine::new(sample_rate);
        let engine_state = engine.state();

        if let Err(e) = engine.start() {
            tracing::error!("Failed to start audio engine: {}", e);
        }

        // Create a default MIDI track and set one-bar loop
        engine.with_timeline(|timeline| {
            timeline.add_track(TrackKind::Midi, "Track 1");
            // Set default loop to one bar (4 beats at current tempo)
            let samples_per_beat = (timeline.transport.sample_rate as f64 * 60.0) / timeline.transport.bpm;
            timeline.transport.loop_start = 0;
            timeline.transport.loop_end = (samples_per_beat * 4.0) as u64; // One bar
            timeline.transport.loop_enabled = true;
        });

        let input_monitor = InputMonitor::new();
        let meter_state = input_monitor.meter_state();

        // Initialize native GUI manager for plugin windows
        let mut gui_manager = PluginGuiManager::new();
        if let Err(e) = gui_manager.initialize() {
            tracing::warn!("Failed to initialize native GUI manager: {}", e);
        }

        // Load config and initialize sample library places
        let config = load_config();
        let mut browser_panel = BrowserPanel::new();
        let place_paths: Vec<PathBuf> = config.library.places.iter().map(PathBuf::from).collect();
        browser_panel.set_places(place_paths);

        Self {
            engine,
            engine_state,
            input_monitor,
            meter_state,
            transport_panel: TransportPanel::new(),
            plugin_menu: PluginBrowserPanel::new(),
            browser_panel,
            track_headers_panel: TrackHeadersPanel::new(),
            arrange_panel: ArrangePanel::new(),
            device_rack_panel: DeviceRackPanel::new(),
            clip_editor_panel: ClipEditorPanel::new(),
            drum_roll_panel: DrumRollPanel::new(),
            pattern_bank_panel: PatternBankPanel::new(),
            keyboard_sequencer_panel: KeyboardSequencerPanel::new(),
            midi_fx_rack_panel: MidiFxRackPanel::new(),
            song_view_panel: SongViewPanel::new(),
            clipboard: DawClipboard::default(),
            show_hapax_panels: true,
            selected_track_idx: Some(0),
            selected_clip: None,
            show_clip_editor: false,
            plugin_windows: Vec::new(),
            native_param_windows: HashSet::new(),
            gui_manager,
            next_clip_id: 1,
            next_instrument_id: 1,
            next_effect_chain_id: 1,
            recording_start_sample: 0,
            playback_start_position: 0,
        }
    }

    fn start_recording(&mut self) {
        if !self.input_monitor.is_running() {
            if let Err(e) = self.input_monitor.start("default") {
                tracing::error!("Failed to start input monitor: {}", e);
                return;
            }
        }

        self.recording_start_sample = self.engine.position();

        if let Err(e) = self.input_monitor.start_recording() {
            tracing::error!("Failed to start recording: {}", e);
            return;
        }

        self.engine.play();
        tracing::info!("Recording started at sample {}", self.recording_start_sample);
    }

    fn stop_recording(&mut self) {
        let Ok(recorded) = self.input_monitor.stop_recording() else {
            tracing::error!("Failed to stop recording");
            return;
        };

        if recorded.samples.is_empty() {
            tracing::warn!("No samples recorded");
            return;
        }

        let mut clip = AudioClip::new(
            ClipId(self.next_clip_id),
            recorded.samples,
            recorded.sample_rate,
            recorded.channels,
        );
        self.next_clip_id += 1;

        clip.start_sample = self.recording_start_sample;
        clip.name = format!("Recording {}", self.next_clip_id - 1);

        // Add to armed track or first track
        self.engine.with_timeline(|timeline| {
            let armed_idx = timeline.tracks.iter().position(|t| t.armed);
            let target_idx = armed_idx.or(if timeline.tracks.is_empty() { None } else { Some(0) });

            if let Some(idx) = target_idx {
                if let Some(track) = timeline.tracks.get_mut(idx) {
                    track.add_clip(clip);
                }
            }
        });
    }

    fn load_audio_file(&mut self, path: &std::path::Path) {
        let Ok(mut reader) = hound::WavReader::open(path) else {
            tracing::error!("Failed to open WAV file: {}", path.display());
            return;
        };

        let spec = reader.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
            hound::SampleFormat::Int => {
                let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
                reader.samples::<i32>()
                    .filter_map(Result::ok)
                    .map(|s| s as f32 / max_val)
                    .collect()
            }
        };

        let mut clip = AudioClip::new(
            ClipId(self.next_clip_id),
            samples,
            spec.sample_rate,
            spec.channels,
        );
        self.next_clip_id += 1;

        clip.name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("clip")
            .to_string();

        self.engine.with_timeline(|timeline| {
            if let Some(track) = timeline.tracks.first_mut() {
                track.add_clip(clip);
            }
        });

        tracing::info!("Loaded audio file: {}", path.display());
    }

    fn add_audio_track(&mut self) {
        let track_idx = self.engine.with_timeline(|timeline| {
            let idx = timeline.tracks.len();
            let name = format!("Audio {}", idx + 1);
            timeline.add_track(TrackKind::Audio, name);
            idx
        });

        if let Some(idx) = track_idx {
            self.selected_track_idx = Some(idx);
        }

        tracing::info!("Added new audio track");
    }

    fn add_empty_midi_track(&mut self) {
        let track_idx = self.engine.with_timeline(|timeline| {
            let idx = timeline.tracks.len();
            let name = format!("MIDI {}", idx + 1);
            timeline.add_track(TrackKind::Midi, name);
            idx
        });

        if let Some(idx) = track_idx {
            self.selected_track_idx = Some(idx);
        }

        tracing::info!("Added new MIDI track");
    }

    fn load_native_drum(&mut self, inst_id_str: &str) {
        // Currently only 808 is supported
        if inst_id_str != "drum808" {
            return;
        }

        let sample_rate = self.engine.sample_rate() as f32;
        let drum808 = Drum808::new(sample_rate);

        let inst_id = self.next_instrument_id;
        self.next_instrument_id += 1;

        self.engine.add_instrument(inst_id, Instrument::Drum808(drum808));

        // Check if a MIDI track is selected - load there, else create new track
        let selected_midi_track = self.selected_track_idx.and_then(|idx| {
            self.engine.with_timeline(|timeline| {
                timeline.tracks.get(idx).and_then(|t| {
                    if t.kind == TrackKind::Midi { Some(idx) } else { None }
                })
            }).flatten()
        });

        let clip_id = self.next_clip_id;
        self.next_clip_id += 1;

        let track_idx = if let Some(idx) = selected_midi_track {
            // Load to existing selected MIDI track
            self.engine.with_timeline(|timeline| {
                if let Some(track) = timeline.tracks.get_mut(idx) {
                    track.instrument_id = Some(inst_id);
                    track.name = "808 Drums".to_string();

                    // Create a 4-bar MIDI clip if track has no clips
                    if track.midi_clips.is_empty() {
                        let sample_rate = timeline.transport.sample_rate;
                        let bpm = timeline.transport.bpm;
                        let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
                        let length_samples = (samples_per_beat * 16.0) as u64;

                        let mut clip = MidiClip::new(ClipId(clip_id), length_samples);
                        clip.name = "Drum Pattern".to_string();
                        track.add_midi_clip(clip);
                    }
                }
            });
            Some(idx)
        } else {
            // Create new MIDI track
            self.engine.with_timeline(|timeline| {
                let idx = timeline.tracks.len();
                timeline.add_track(TrackKind::Midi, "808 Drums");

                let Some(track) = timeline.tracks.last_mut() else { return Some(idx) };
                track.instrument_id = Some(inst_id);

                let sample_rate = timeline.transport.sample_rate;
                let bpm = timeline.transport.bpm;
                let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
                let length_samples = (samples_per_beat * 16.0) as u64;

                let mut clip = MidiClip::new(ClipId(clip_id), length_samples);
                clip.name = "Drum Pattern".to_string();
                track.add_midi_clip(clip);

                Some(idx)
            }).flatten()
        };

        if let Some(idx) = track_idx {
            self.selected_track_idx = Some(idx);
            self.selected_clip = Some(SelectedClip::Midi {
                track_idx: idx,
                clip_id: ClipId(clip_id),
            });
            self.show_clip_editor = true;
        }

        tracing::info!("Loaded 808 Drum Machine");
    }

    fn save_library_config(&self) {
        let config = AppConfig {
            library: LibraryConfig {
                places: self.browser_panel.place_paths().iter().map(|p| p.display().to_string()).collect(),
            },
        };
        save_config(&config);
    }

    fn load_sample(&mut self, path: &std::path::Path) {
        let engine_sr = self.engine.sample_rate() as f32;

        let sampler = match Sampler::from_wav(path, engine_sr) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to load sample: {}", e);
                return;
            }
        };

        let inst_name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Sample")
            .to_string();
        let inst_id = self.next_instrument_id;
        self.next_instrument_id += 1;

        self.engine.add_instrument(inst_id, Instrument::Sampler(sampler));

        // Use selected MIDI track or create a new one (same pattern as load_native_drum)
        let selected_midi_track = self.selected_track_idx.and_then(|idx| {
            self.engine.with_timeline(|timeline| {
                timeline.tracks.get(idx).and_then(|t| {
                    if t.kind == TrackKind::Midi { Some(idx) } else { None }
                })
            }).flatten()
        });

        let clip_id = self.next_clip_id;
        self.next_clip_id += 1;

        let track_idx = if let Some(idx) = selected_midi_track {
            self.engine.with_timeline(|timeline| {
                if let Some(track) = timeline.tracks.get_mut(idx) {
                    track.instrument_id = Some(inst_id);
                    track.name = format!("Sample - {}", inst_name);

                    if track.midi_clips.is_empty() {
                        let sample_rate = timeline.transport.sample_rate;
                        let bpm = timeline.transport.bpm;
                        let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
                        let length_samples = (samples_per_beat * 16.0) as u64;

                        let mut clip = MidiClip::new(ClipId(clip_id), length_samples);
                        clip.name = "Sample Pattern".to_string();
                        track.add_midi_clip(clip);
                    }
                }
            });
            Some(idx)
        } else {
            self.engine.with_timeline(|timeline| {
                let idx = timeline.tracks.len();
                timeline.add_track(TrackKind::Midi, format!("Sample - {}", inst_name));

                let Some(track) = timeline.tracks.last_mut() else { return Some(idx) };
                track.instrument_id = Some(inst_id);

                let sample_rate = timeline.transport.sample_rate;
                let bpm = timeline.transport.bpm;
                let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
                let length_samples = (samples_per_beat * 16.0) as u64;

                let mut clip = MidiClip::new(ClipId(clip_id), length_samples);
                clip.name = "Sample Pattern".to_string();
                track.add_midi_clip(clip);

                Some(idx)
            }).flatten()
        };

        if let Some(idx) = track_idx {
            self.selected_track_idx = Some(idx);
            self.selected_clip = Some(SelectedClip::Midi {
                track_idx: idx,
                clip_id: ClipId(clip_id),
            });
            self.show_clip_editor = true;
        }

        tracing::info!("Loaded sample: {}", path.display());
    }

    fn load_vst3_effect(&mut self, info: &Vst3PluginInfo) {
        let Some(scanner) = self.plugin_menu.scanner() else { return };
        let Some(rack_scanner) = scanner.scanner() else { return };

        let sample_rate = self.engine.sample_rate() as f32;

        match Vst3Effect::new(rack_scanner, info, sample_rate) {
            Ok(effect) => {
                tracing::info!("Loaded VST3 effect: {}", info.name);
                self.engine.with_master_effects(|chain| {
                    chain.add(Box::new(effect));
                });
            }
            Err(e) => {
                tracing::error!("Failed to load VST3 plugin {}: {}", info.name, e);
            }
        }
    }

    fn load_instrument_to_track(&mut self, info: &Vst3PluginInfo) {

        let Some(scanner) = self.plugin_menu.scanner() else {
            tracing::warn!("No scanner available");
            return;
        };
        let Some(rack_scanner) = scanner.scanner() else {
            tracing::warn!("No rack scanner available");
            return;
        };

        let sample_rate = self.engine.sample_rate() as f32;

        let instrument = match Vst3Instrument::new(rack_scanner, info, sample_rate) {
            Ok(inst) => inst,
            Err(e) => {
                tracing::error!("Failed to load VST3 instrument {}: {}", info.name, e);
                return;
            }
        };

        let inst_id = self.next_instrument_id;
        self.next_instrument_id += 1;

        tracing::info!("Adding instrument with inst_id={}", inst_id);
        self.engine.add_instrument(inst_id, Instrument::Vst3(instrument));

        // Check if we have a selected MIDI track to add the instrument to
        let selected_midi_track = self.selected_track_idx.and_then(|idx| {
            self.engine.with_timeline(|timeline| {
                timeline.tracks.get(idx).and_then(|track| {
                    if track.kind == TrackKind::Midi {
                        Some(idx)
                    } else {
                        None
                    }
                })
            }).flatten()
        });

        if let Some(track_idx) = selected_midi_track {
            // Add instrument to existing selected MIDI track
            let clip_id = self.engine.with_timeline(|timeline| {
                let Some(track) = timeline.tracks.get_mut(track_idx) else { return None };
                track.instrument_id = Some(inst_id);
                track.name = format!("MIDI - {}", info.name);

                // Ensure track has a MIDI clip, create one if not
                if track.midi_clips.is_empty() {
                    let new_clip_id = ClipId(self.next_clip_id);
                    let sample_rate = timeline.transport.sample_rate;
                    let bpm = timeline.transport.bpm;
                    let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
                    let length_samples = (samples_per_beat * 16.0) as u64;

                    let mut clip = MidiClip::new(new_clip_id, length_samples);
                    clip.name = "New MIDI Clip".to_string();
                    track.add_midi_clip(clip);
                    Some(new_clip_id)
                } else {
                    track.midi_clips.first().map(|c| c.id)
                }
            }).flatten();

            // Update clip ID counter if we created a new clip
            if clip_id == Some(ClipId(self.next_clip_id)) {
                self.next_clip_id += 1;
            }

            // Select the clip and open piano roll
            if let Some(cid) = clip_id {
                self.selected_clip = Some(SelectedClip::Midi {
                    track_idx,
                    clip_id: cid,
                });
                self.show_clip_editor = true;
            }

            // Open native plugin GUI automatically
            self.open_native_plugin_gui(inst_id, &info.info.path.to_string_lossy(), &info.info.unique_id, &info.name);
            tracing::info!("Added instrument {} to track {}", info.name, track_idx);
        } else {
            // Create new MIDI track with instrument
            let clip_id = self.next_clip_id;
            self.next_clip_id += 1;

            let track_idx = self.engine.with_timeline(|timeline| {
                let idx = timeline.tracks.len();
                timeline.add_track(TrackKind::Midi, format!("MIDI - {}", info.name));

                let Some(track) = timeline.tracks.last_mut() else { return idx };
                track.instrument_id = Some(inst_id);

                let sample_rate = timeline.transport.sample_rate;
                let bpm = timeline.transport.bpm;
                let samples_per_beat = sample_rate as f64 * 60.0 / bpm;
                let length_samples = (samples_per_beat * 16.0) as u64;

                let mut clip = MidiClip::new(ClipId(clip_id), length_samples);
                clip.name = "New MIDI Clip".to_string();
                track.add_midi_clip(clip);

                idx
            });

            if let Some(idx) = track_idx {
                self.selected_track_idx = Some(idx);
                self.selected_clip = Some(SelectedClip::Midi {
                    track_idx: idx,
                    clip_id: ClipId(clip_id),
                });
                self.show_clip_editor = true;

                // Open native plugin GUI automatically
                self.open_native_plugin_gui(inst_id, &info.info.path.to_string_lossy(), &info.info.unique_id, &info.name);
            }

            tracing::info!("Created MIDI track with instrument: {}", info.name);
        }
    }

    fn handle_track_header_action(&mut self, action: TrackHeaderAction) {
        match action {
            TrackHeaderAction::SelectTrack(idx) => {
                self.selected_track_idx = Some(idx);
            }
            TrackHeaderAction::ToggleMute(idx) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(idx) {
                        track.mute = !track.mute;
                    }
                });
            }
            TrackHeaderAction::ToggleSolo(idx) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(idx) {
                        track.solo = !track.solo;
                    }
                });
            }
            TrackHeaderAction::ToggleArm(idx) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(idx) {
                        track.armed = !track.armed;
                    }
                });
            }
            TrackHeaderAction::SetVolume(idx, vol) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(idx) {
                        track.volume = vol;
                    }
                });
            }
            TrackHeaderAction::SetPan(idx, pan) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(idx) {
                        track.pan = pan;
                    }
                });
            }
            TrackHeaderAction::RenameTrack(idx, name) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(idx) {
                        track.name = name;
                    }
                });
            }
            TrackHeaderAction::DeleteTrack(idx) => {
                self.engine.with_timeline(|timeline| {
                    if idx < timeline.tracks.len() {
                        timeline.tracks.remove(idx);
                    }
                });
                // Clear selection if deleted track was selected
                if self.selected_track_idx == Some(idx) {
                    self.selected_track_idx = None;
                    self.selected_clip = None;
                }
            }
            TrackHeaderAction::AddAudioTrack => {
                self.add_audio_track();
            }
            TrackHeaderAction::AddMidiTrack => {
                self.add_empty_midi_track();
            }
            TrackHeaderAction::None => {}
        }
    }

    fn handle_arrange_action(&mut self, action: ArrangeAction) {
        match action {
            ArrangeAction::SelectClip { track_idx, clip_id } => {
                self.selected_track_idx = Some(track_idx);

                // Determine clip type
                let clip_type = self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get(track_idx) {
                        if track.clips.iter().any(|c| c.id == clip_id) {
                            return Some(false); // Audio
                        }
                        if track.midi_clips.iter().any(|c| c.id == clip_id) {
                            return Some(true); // MIDI
                        }
                    }
                    None
                }).flatten();

                self.selected_clip = clip_type.map(|is_midi| {
                    if is_midi {
                        SelectedClip::Midi { track_idx, clip_id }
                    } else {
                        SelectedClip::Audio { track_idx, clip_id }
                    }
                });
            }
            ArrangeAction::OpenClipEditor { track_idx, clip_id } => {
                self.selected_track_idx = Some(track_idx);

                let clip_type = self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get(track_idx) {
                        if track.clips.iter().any(|c| c.id == clip_id) {
                            return Some(false);
                        }
                        if track.midi_clips.iter().any(|c| c.id == clip_id) {
                            return Some(true);
                        }
                    }
                    None
                }).flatten();

                self.selected_clip = clip_type.map(|is_midi| {
                    if is_midi {
                        SelectedClip::Midi { track_idx, clip_id }
                    } else {
                        SelectedClip::Audio { track_idx, clip_id }
                    }
                });

                self.show_clip_editor = true;
            }
            ArrangeAction::Seek(samples) => {
                self.engine.seek(samples);
            }
            ArrangeAction::AddAudioTrack => {
                self.add_audio_track();
            }
            ArrangeAction::AddMidiTrack => {
                self.add_empty_midi_track();
            }
            ArrangeAction::SetLoopRegion { start_sample, end_sample } => {
                self.engine.set_loop_region(start_sample, end_sample);
                self.engine.set_loop_enabled(true);
            }
            ArrangeAction::None => {}
        }
    }

    fn handle_device_rack_action(&mut self, action: DeviceRackAction) {
        match action {
            DeviceRackAction::OpenPluginWindow(id) => {
                tracing::info!("OpenPluginWindow action for id={}", id);

                // Check if native window already exists
                if self.gui_manager.has_window(id) {
                    // Just show the existing window
                    if let Err(e) = self.gui_manager.show_window(id) {
                        tracing::warn!("Failed to show plugin window: {}", e);
                    }
                    return;
                }

                // Get plugin info from instruments (VST3 only)
                let plugin_info = {
                    let instruments = self.engine_state.instruments.lock().unwrap();
                    instruments.get(&id).and_then(|inst| {
                        inst.vst3_plugin_info().map(|info| (
                            info.name.clone(),
                            info.info.path.to_string_lossy().to_string(),
                            info.info.unique_id.clone(),
                        ))
                    })
                };

                let Some((title, plugin_path, plugin_uid)) = plugin_info else {
                    // Not a VST3 plugin - check if it's a native instrument
                    let is_native = {
                        let instruments = self.engine_state.instruments.lock().unwrap();
                        let result = instruments.get(&id).map(|i| i.is_drum()).unwrap_or(false);
                        tracing::info!("Checking if id={} is native: {}", id, result);
                        result
                    };
                    if is_native {
                        // Toggle native param window
                        if self.native_param_windows.contains(&id) {
                            tracing::info!("Closing native param window for id={}", id);
                            self.native_param_windows.remove(&id);
                        } else {
                            tracing::info!("Opening native param window for id={}", id);
                            self.native_param_windows.insert(id);
                        }
                    }
                    return;
                };

                // Open native plugin GUI directly
                self.open_native_plugin_gui(id, &plugin_path, &plugin_uid, &title);
            }
            DeviceRackAction::ToggleBypass(_id) => {
                // TODO: Implement bypass toggle
            }
            DeviceRackAction::RemoveDevice(_id) => {
                // TODO: Implement device removal
            }
            DeviceRackAction::AddEffect => {
                // TODO: Open effect browser
            }
            DeviceRackAction::None => {}
        }
    }

    fn get_device_info_for_track(&self, track_idx: usize) -> (Option<DeviceInfo>, Vec<DeviceInfo>) {
        let inst_id = self.engine.with_timeline(|timeline| {
            timeline.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();

        let instrument = inst_id.map(|id| {
            let instruments = self.engine_state.instruments.lock().unwrap();
            let name = instruments.get(&id)
                .map(|i| i.name().to_string())
                .unwrap_or_else(|| "Instrument".to_string());
            DeviceInfo {
                id,
                name,
                is_instrument: true,
                is_bypassed: false,
                has_ui: true,
            }
        });

        // TODO: Get actual effect chain from track
        let effects = Vec::new();

        (instrument, effects)
    }

    fn get_plugins(&self) -> Vec<Vst3PluginInfo> {
        self.plugin_menu.scanner()
            .map(|s| s.plugins().to_vec())
            .unwrap_or_default()
    }

    fn handle_pattern_bank_action(&mut self, action: PatternBankAction) {
        let Some(track_idx) = self.selected_track_idx else { return };

        match action {
            PatternBankAction::SelectPattern(idx) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        track.pattern_bank.set_active(idx);
                    }
                });
            }
            PatternBankAction::QueuePattern(idx) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        track.pattern_bank.queue_pattern(idx);
                    }
                });
            }
            PatternBankAction::EditPattern(_idx) => {
                // Open the pattern in the clip editor
                self.show_clip_editor = true;
            }
            PatternBankAction::CopyPattern { from, to } => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        track.pattern_bank.copy_pattern(from, to);
                    }
                });
            }
            PatternBankAction::ClearPattern(idx) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        track.pattern_bank.clear_pattern(idx);
                    }
                });
            }
            PatternBankAction::None => {}
        }
    }

    fn handle_keyboard_sequencer_actions(&mut self, actions: Vec<KeyboardSequencerAction>) {
        let Some(track_idx) = self.selected_track_idx else { return };

        for action in actions {
            match action {
                KeyboardSequencerAction::ToggleDrumStep(step) => {
                    tracing::debug!("Toggle drum step {}", step);
                }
                KeyboardSequencerAction::PlayNote { pitch, velocity } => {
                    let inst_id = self.engine.with_timeline(|t| {
                        t.tracks.get(track_idx).and_then(|track| track.instrument_id)
                    }).flatten();
                    if let Some(id) = inst_id {
                        let mut instruments = self.engine_state.instruments.lock().unwrap();
                        if let Some(inst) = instruments.get_mut(&id) {
                            inst.queue_note_on(pitch, velocity, 0, 0);
                        }
                    }
                }
                KeyboardSequencerAction::StopNote { pitch } => {
                    let inst_id = self.engine.with_timeline(|t| {
                        t.tracks.get(track_idx).and_then(|track| track.instrument_id)
                    }).flatten();
                    if let Some(id) = inst_id {
                        let mut instruments = self.engine_state.instruments.lock().unwrap();
                        if let Some(inst) = instruments.get_mut(&id) {
                            inst.queue_note_off(pitch, 0, 0, 0);
                        }
                    }
                }
                KeyboardSequencerAction::LoadStepSample { step, path } => {
                    tracing::debug!("LoadStepSample step={} path={:?}", step, path);
                    self.load_step_sample(track_idx, step, &path);
                }
                KeyboardSequencerAction::PlayDrumStep { step, velocity } => {
                    let inst_id = self.engine.with_timeline(|t| {
                        t.tracks.get(track_idx).and_then(|track| track.instrument_id)
                    }).flatten();
                    if let Some(id) = inst_id {
                        let mut instruments = self.engine_state.instruments.lock().unwrap();
                        if let Some(inst) = instruments.get_mut(&id) {
                            inst.queue_note_on(36 + step as u8, velocity, 0, 0);
                        }
                    }
                }
                KeyboardSequencerAction::CopyStepSample { from, to } => {
                    self.copy_step_sample(track_idx, from, to);
                }
                KeyboardSequencerAction::CopyDrumStep(step) => {
                    tracing::debug!("CopyDrumStep step={}", step);
                    self.copy_drum_step_to_clipboard(track_idx, step);
                }
                KeyboardSequencerAction::PasteStepSample { step, name, data } => {
                    tracing::debug!("PasteStepSample step={} name={}", step, name);
                    self.paste_step_sample(track_idx, step, name, data);
                }
                KeyboardSequencerAction::None => {}
            }
        }
    }

    fn copy_step_sample(&mut self, track_idx: usize, from: usize, to: usize) {
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();
        let Some(id) = inst_id else { return };

        let mut instruments = self.engine_state.instruments.lock().unwrap();
        let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&id) else { return };

        let (name, data) = {
            let slots = kit.slots();
            let Some(slot) = slots.get(from).and_then(|s| s.as_ref()) else { return };
            (slot.name.clone(), Arc::clone(&slot.data))
        };

        kit.set_slot(to, name.clone(), data);
        drop(instruments);

        self.keyboard_sequencer_panel.set_step_sample_name(to, name);
    }

    fn copy_drum_step_to_clipboard(&mut self, track_idx: usize, step: usize) {
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();
        let Some(id) = inst_id else { return };

        let instruments = self.engine_state.instruments.lock().unwrap();
        let Some(Instrument::SampleKit(kit)) = instruments.get(&id) else { return };
        let slots = kit.slots();
        let Some(slot) = slots.get(step).and_then(|s| s.as_ref()) else { return };

        self.clipboard.copy(ClipboardContent::SampleData {
            name: slot.name.clone(),
            data: Arc::clone(&slot.data),
        });
    }

    fn paste_step_sample(
        &mut self,
        track_idx: usize,
        step: usize,
        name: String,
        data: Arc<Vec<f32>>,
    ) {
        let engine_sr = self.engine.sample_rate() as f32;

        // Get or create SampleKit
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();

        let kit_id = if let Some(id) = inst_id {
            let is_kit = {
                let instruments = self.engine_state.instruments.lock().unwrap();
                instruments.get(&id).map(|i| matches!(i, Instrument::SampleKit(_))).unwrap_or(false)
            };
            if is_kit { id } else {
                let new_id = self.next_instrument_id;
                self.next_instrument_id += 1;
                self.engine.add_instrument(new_id, Instrument::SampleKit(SampleKit::new(engine_sr)));
                self.engine.with_timeline(|t| {
                    if let Some(track) = t.tracks.get_mut(track_idx) {
                        track.instrument_id = Some(new_id);
                    }
                });
                new_id
            }
        } else {
            let new_id = self.next_instrument_id;
            self.next_instrument_id += 1;
            self.engine.add_instrument(new_id, Instrument::SampleKit(SampleKit::new(engine_sr)));
            self.engine.with_timeline(|t| {
                if let Some(track) = t.tracks.get_mut(track_idx) {
                    track.instrument_id = Some(new_id);
                }
            });
            new_id
        };

        {
            let mut instruments = self.engine_state.instruments.lock().unwrap();
            if let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&kit_id) {
                kit.set_slot(step, name.clone(), data);
            }
        }

        self.keyboard_sequencer_panel.set_step_sample_name(step, name);
    }

    fn load_step_sample(&mut self, track_idx: usize, step: usize, path: &std::path::Path) {
        let engine_sr = self.engine.sample_rate() as f32;

        let (mono, _sample_rate) = match Self::read_wav_samples(path) {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Failed to read WAV: {} — {}", path.display(), e);
                return;
            }
        };

        let sample_name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("sample")
            .to_string();

        let data = Arc::new(mono);

        // Get or create SampleKit instrument for this track
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();

        let kit_id = if let Some(id) = inst_id {
            // Check if existing instrument is already a SampleKit
            let is_kit = {
                let instruments = self.engine_state.instruments.lock().unwrap();
                instruments.get(&id).map(|i| matches!(i, Instrument::SampleKit(_))).unwrap_or(false)
            };
            if is_kit { id } else {
                // Replace with a new SampleKit
                let new_id = self.next_instrument_id;
                self.next_instrument_id += 1;
                self.engine.add_instrument(new_id, Instrument::SampleKit(SampleKit::new(engine_sr)));
                self.engine.with_timeline(|t| {
                    if let Some(track) = t.tracks.get_mut(track_idx) {
                        track.instrument_id = Some(new_id);
                    }
                });
                new_id
            }
        } else {
            // No instrument on track — create SampleKit
            let new_id = self.next_instrument_id;
            self.next_instrument_id += 1;
            self.engine.add_instrument(new_id, Instrument::SampleKit(SampleKit::new(engine_sr)));
            self.engine.with_timeline(|t| {
                if let Some(track) = t.tracks.get_mut(track_idx) {
                    track.instrument_id = Some(new_id);
                }
            });
            new_id
        };

        // Assign sample to slot
        {
            let mut instruments = self.engine_state.instruments.lock().unwrap();
            if let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&kit_id) {
                kit.set_slot(step, sample_name.clone(), data);
            }
        }

        // Update sequencer display
        self.keyboard_sequencer_panel.set_step_sample_name(step, sample_name);
        tracing::info!("Loaded sample to step {}: {}", step, path.display());
    }

    /// Read a WAV file to mono f32 samples. Tries hound first, falls back to
    /// manual RIFF parsing for files with extended fmt chunks.
    fn read_wav_samples(path: &std::path::Path) -> Result<(Vec<f32>, u32), String> {
        // Try hound first (fast path for standard WAV)
        if let Ok(mut reader) = hound::WavReader::open(path) {
            let spec = reader.spec();
            let samples: Vec<f32> = match spec.sample_format {
                hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
                hound::SampleFormat::Int => {
                    let max_val = (1u64 << (spec.bits_per_sample - 1)) as f32;
                    reader.samples::<i32>().filter_map(Result::ok).map(|s| s as f32 / max_val).collect()
                }
            };
            let mono = to_mono(&samples, spec.channels as usize);
            return Ok((mono, spec.sample_rate));
        }

        // Fallback: manual RIFF/WAVE parser (handles extended fmt chunks)
        Self::read_wav_manual(path)
    }

    fn read_wav_manual(path: &std::path::Path) -> Result<(Vec<f32>, u32), String> {
        use std::io::{Read, Seek, SeekFrom};

        let mut f = std::fs::File::open(path).map_err(|e| format!("open: {e}"))?;
        let mut buf4 = [0u8; 4];
        let mut buf2 = [0u8; 2];

        // RIFF header
        f.read_exact(&mut buf4).map_err(|e| format!("read RIFF: {e}"))?;
        if &buf4 != b"RIFF" { return Err("not RIFF".into()); }
        f.read_exact(&mut buf4).ok(); // file size, skip
        f.read_exact(&mut buf4).map_err(|e| format!("read WAVE: {e}"))?;
        if &buf4 != b"WAVE" { return Err("not WAVE".into()); }

        let mut sample_rate = 0u32;
        let mut channels = 0u16;
        let mut bits_per_sample = 0u16;
        let mut audio_format = 0u16;
        let mut data_bytes: Vec<u8> = Vec::new();

        // Walk chunks
        loop {
            let Ok(()) = f.read_exact(&mut buf4) else { break };
            let chunk_id = buf4;
            let Ok(()) = f.read_exact(&mut buf4) else { break };
            let chunk_size = u32::from_le_bytes(buf4);

            if &chunk_id == b"fmt " {
                f.read_exact(&mut buf2).map_err(|e| format!("fmt: {e}"))?;
                audio_format = u16::from_le_bytes(buf2);
                f.read_exact(&mut buf2).map_err(|e| format!("fmt: {e}"))?;
                channels = u16::from_le_bytes(buf2);
                f.read_exact(&mut buf4).map_err(|e| format!("fmt: {e}"))?;
                sample_rate = u32::from_le_bytes(buf4);
                f.read_exact(&mut buf4).ok(); // byte rate
                f.read_exact(&mut buf2).ok(); // block align
                f.read_exact(&mut buf2).map_err(|e| format!("fmt: {e}"))?;
                bits_per_sample = u16::from_le_bytes(buf2);
                // Skip remaining fmt bytes (extended chunk)
                let read_so_far = 16u32;
                if chunk_size > read_so_far {
                    f.seek(SeekFrom::Current((chunk_size - read_so_far) as i64)).ok();
                }
                continue;
            }

            if &chunk_id == b"data" {
                data_bytes.resize(chunk_size as usize, 0);
                f.read_exact(&mut data_bytes).map_err(|e| format!("data: {e}"))?;
                break;
            }

            // Skip unknown chunk
            f.seek(SeekFrom::Current(chunk_size as i64)).ok();
        }

        if data_bytes.is_empty() { return Err("no data chunk".into()); }
        if audio_format != 1 { return Err(format!("unsupported format {audio_format}")); }

        let samples: Vec<f32> = match bits_per_sample {
            16 => data_bytes.chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                .collect(),
            24 => data_bytes.chunks_exact(3)
                .map(|b| {
                    let val = (b[0] as i32) | ((b[1] as i32) << 8) | ((b[2] as i32) << 16);
                    let signed = if val & 0x800000 != 0 { val | !0xFFFFFF } else { val };
                    signed as f32 / 8388608.0
                })
                .collect(),
            32 => data_bytes.chunks_exact(4)
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2147483648.0)
                .collect(),
            _ => return Err(format!("unsupported bits {bits_per_sample}")),
        };

        let mono = to_mono(&samples, channels as usize);
        Ok((mono, sample_rate))
    }

    fn handle_midi_fx_rack_action(&mut self, action: MidiFxRackAction) {
        let Some(track_idx) = self.selected_track_idx else { return };

        match action {
            MidiFxRackAction::AddEffect(effect_type) => {
                let effect = effect_type.create_effect();
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        track.midi_fx_chain.add(effect);
                    }
                });
            }
            MidiFxRackAction::RemoveEffect(idx) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        track.midi_fx_chain.remove(idx);
                    }
                });
            }
            MidiFxRackAction::ToggleBypass(idx) => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        if let Some(effect) = track.midi_fx_chain.effects.get_mut(idx) {
                            let bypassed = effect.is_bypassed();
                            effect.set_bypass(!bypassed);
                        }
                    }
                });
            }
            MidiFxRackAction::MoveEffect { from, to } => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        let effects = &mut track.midi_fx_chain.effects;
                        if from < effects.len() && to < effects.len() {
                            let effect = effects.remove(from);
                            effects.insert(to, effect);
                        }
                    }
                });
            }
            MidiFxRackAction::SetParam { effect_idx, param_name, value } => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        if let Some(effect) = track.midi_fx_chain.effects.get_mut(effect_idx) {
                            effect.set_param(&param_name, value);
                        }
                    }
                });
            }
            MidiFxRackAction::None => {}
        }
    }

    fn handle_song_view_action(&mut self, action: SongViewAction) {
        match action {
            SongViewAction::SelectSection(_idx) => {
                // Update selection - would need song arrangement in project
            }
            SongViewAction::AddSection => {
                // Add new section
            }
            SongViewAction::RemoveSection(_idx) => {
                // Remove section
            }
            SongViewAction::DuplicateSection(_idx) => {
                // Duplicate section
            }
            SongViewAction::MoveSection { from: _, to: _ } => {
                // Move section
            }
            SongViewAction::SetSectionLength { index: _, bars: _ } => {
                // Set section length
            }
            SongViewAction::SetSectionRepeat { index: _, count: _ } => {
                // Set section repeat count
            }
            SongViewAction::SetPlaybackMode(_mode) => {
                // Toggle pattern/song mode
            }
            SongViewAction::JumpToSection(_idx) => {
                // Jump playhead to section
            }
            SongViewAction::None => {}
        }
    }

    /// Open native plugin GUI window directly (no egui parameter window)
    fn open_native_plugin_gui(&mut self, plugin_id: u64, plugin_path: &str, plugin_uid: &str, title: &str) {
        if plugin_path.is_empty() || plugin_uid.is_empty() {
            tracing::warn!("Cannot open native GUI: missing plugin path or UID");
            return;
        }

        tracing::info!("Opening native GUI for plugin_id={} title={}", plugin_id, title);

        // Create native window with VST3 plugin view attached
        match self.gui_manager.create_window(plugin_id, plugin_path, plugin_uid, title, 800, 600) {
            Ok(()) => {
                // Show the window immediately
                if let Err(e) = self.gui_manager.show_window(plugin_id) {
                    tracing::warn!("Failed to show native plugin window: {}", e);
                } else {
                    tracing::info!("Opened native GUI for plugin {}", title);
                }
            }
            Err(e) => {
                tracing::error!("Failed to create native plugin window: {}", e);
            }
        }
    }
}

impl eframe::App for SignumApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Handle dropped files
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                if let Some(path) = &file.path {
                    if path.extension().is_some_and(|e| e == "wav") {
                        self.load_audio_file(path);
                    }
                }
            }
        });

        // Global spacebar → toggle playback (skip if a text field is focused)
        let text_focused = ctx.memory(|mem| mem.focused().is_some())
            && ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(_))));
        if !text_focused && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            if self.engine.is_playing() {
                self.engine.pause();
                self.engine.seek(self.playback_start_position);
            } else {
                self.playback_start_position = self.engine.position();
                self.engine.play();
            }
        }

        // 1. Menu bar
        let plugin_action = egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.plugin_menu.menu_ui(ui, &mut self.arrange_panel.snap_to_grid)
        }).inner;

        match plugin_action {
            PluginAction::LoadPlugin(info) => self.load_vst3_effect(&info),
            PluginAction::CreateMidiTrack(info) => self.load_instrument_to_track(&info),
            PluginAction::AddAudioTrack => self.add_audio_track(),
            PluginAction::AddMidiTrack => self.add_empty_midi_track(),
            PluginAction::None => {}
        }

        // 2. Transport bar
        let transport_action = egui::TopBottomPanel::top("transport").show(ctx, |ui| {
            self.transport_panel.ui(ui, &self.engine, &self.engine_state, &mut self.input_monitor, &self.meter_state)
        }).inner;

        match transport_action {
            TransportAction::StartRecording => self.start_recording(),
            TransportAction::StopRecording => self.stop_recording(),
            TransportAction::None => {}
        }

        // 3. Bottom panel: Device Rack (always visible, at very bottom)
        egui::TopBottomPanel::bottom("device_rack_panel")
            .resizable(false)
            .exact_height(110.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let track_name = self.selected_track_idx.and_then(|idx| {
                        self.engine.with_timeline(|t| {
                            t.tracks.get(idx).map(|track| track.name.clone())
                        }).flatten()
                    });

                    let (instrument, effects) = self.selected_track_idx
                        .map(|idx| self.get_device_info_for_track(idx))
                        .unwrap_or((None, Vec::new()));

                    let action = self.device_rack_panel.ui(
                        ui,
                        track_name.as_deref(),
                        instrument.clone(),
                        &effects,
                    );
                    if !matches!(action, DeviceRackAction::None) {
                        tracing::info!("Device rack action: {:?}, instrument: {:?}",
                            match &action {
                                DeviceRackAction::OpenPluginWindow(id) => format!("OpenPluginWindow({})", id),
                                DeviceRackAction::ToggleBypass(id) => format!("ToggleBypass({})", id),
                                DeviceRackAction::RemoveDevice(id) => format!("RemoveDevice({})", id),
                                DeviceRackAction::AddEffect => "AddEffect".to_string(),
                                DeviceRackAction::None => "None".to_string(),
                            },
                            instrument.as_ref().map(|i| format!("id={} name={}", i.id, i.name))
                        );
                    }
                    self.handle_device_rack_action(action);
                });
            });

        // 3b. Hapax Sequencer panel (above device rack)
        if self.show_hapax_panels {
            egui::TopBottomPanel::bottom("hapax_panel")
                .resizable(true)
                .default_height(160.0)
                .min_height(100.0)
                .show(ctx, |ui| {
                    // Get selected track info
                    let (track_name, patterns, midi_fx_chain) = self.selected_track_idx
                        .and_then(|idx| {
                            self.engine.with_timeline(|timeline| {
                                timeline.tracks.get(idx).map(|track| {
                                    (
                                        Some(track.name.clone()),
                                        Some(track.pattern_bank.patterns.clone()),
                                        Some(track.midi_fx_chain.clone()),
                                    )
                                })
                            }).flatten()
                        })
                        .unwrap_or((None, None, None));

                    ui.horizontal(|ui| {
                        ui.heading("Sequencer");
                        if let Some(name) = &track_name {
                            ui.separator();
                            ui.label(name);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Hide").clicked() {
                                self.show_hapax_panels = false;
                            }
                            if ui.small_button("?").on_hover_text("Open docs/sequencer.md").clicked() {
                                let _ = open::that("docs/sequencer.md");
                            }
                        });
                    });
                    ui.separator();

                    // Get transport state for sequencer sync
                    let (bpm, sample_rate) = self.engine.with_timeline(|t| {
                        (t.transport.bpm, t.transport.sample_rate)
                    }).unwrap_or((120.0, 44100));
                    let playback_position = self.engine.position();
                    let is_playing = self.engine.is_playing();

                    // Horizontal layout: Keyboard (if docked) | MIDI FX | Song
                    ui.horizontal(|ui| {
                        // Keyboard Sequencer (left, only when docked)
                        if !self.keyboard_sequencer_panel.is_floating {
                            ui.group(|ui| {
                                ui.set_min_width(400.0);
                                let actions = self.keyboard_sequencer_panel.ui(
                                    ui,
                                    track_name.as_deref(),
                                    playback_position,
                                    bpm,
                                    sample_rate,
                                    is_playing,
                                    &self.clipboard,
                                );
                                self.handle_keyboard_sequencer_actions(actions);
                            });
                            ui.separator();
                        }

                        // MIDI FX
                        ui.group(|ui| {
                            ui.set_min_width(280.0);
                            egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                                let action = self.midi_fx_rack_panel.ui(
                                    ui,
                                    track_name.as_deref(),
                                    midi_fx_chain.as_ref(),
                                );
                                self.handle_midi_fx_rack_action(action);
                            });
                        });

                        ui.separator();

                        // Song View (right)
                        ui.group(|ui| {
                            ui.set_min_width(200.0);
                            let (sections, current_section, playback_mode) = (
                                vec![SongSection::default()],
                                0usize,
                                PlaybackMode::Pattern,
                            );
                            let action = self.song_view_panel.ui(
                                ui,
                                &sections,
                                current_section,
                                playback_mode,
                            );
                            self.handle_song_view_action(action);
                        });
                    });
                });
        } else {
            // Show minimal bar to re-open
            egui::TopBottomPanel::bottom("hapax_toggle")
                .resizable(false)
                .exact_height(20.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("▲ Show Sequencer").clicked() {
                            self.show_hapax_panels = true;
                        }
                    });
                });
        }

        // 3c. Floating keyboard sequencer window
        if self.keyboard_sequencer_panel.is_floating {
            let (bpm, sample_rate) = self.engine.with_timeline(|t| {
                (t.transport.bpm, t.transport.sample_rate)
            }).unwrap_or((120.0, 44100));
            let playback_position = self.engine.position();
            let is_playing = self.engine.is_playing();
            let track_name: Option<String> = self.selected_track_idx.and_then(|idx| {
                self.engine.with_timeline(|t| t.tracks.get(idx).map(|tr| tr.name.clone())).flatten()
            });

            let mut still_open = true;
            egui::Window::new("HAPAX Sequencer")
                .open(&mut still_open)
                .resizable(true)
                .default_size([1020.0, 460.0])
                .show(ctx, |ui| {
                    let actions = self.keyboard_sequencer_panel.ui(
                        ui,
                        track_name.as_deref(),
                        playback_position,
                        bpm,
                        sample_rate,
                        is_playing,
                        &self.clipboard,
                    );
                    self.handle_keyboard_sequencer_actions(actions);
                });
            if !still_open {
                self.keyboard_sequencer_panel.is_floating = false;
            }
        }

        // 4. Clip Editor / Piano Roll panel (above sequencer, only when clip selected)
        if self.show_clip_editor {
            egui::TopBottomPanel::bottom("clip_editor_panel")
                .resizable(true)
                .default_height(200.0)
                .min_height(100.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("✕ Close").clicked() {
                            self.show_clip_editor = false;
                        }
                        ui.separator();
                    });

                    let mut piano_roll_action = PianoRollAction::None;
                    let mut drum_roll_action = DrumRollAction::None;

                    if let Some(selected) = self.selected_clip {
                        match selected {
                            SelectedClip::Midi { track_idx, clip_id } => {
                                let (bpm, sample_rate) = self.engine.with_timeline(|t| {
                                    (t.transport.bpm, t.transport.sample_rate)
                                }).unwrap_or((120.0, 44100));

                                let playback_position = self.engine.position();

                                // Check if this track uses a drum instrument
                                let is_drum = self.engine.with_timeline(|timeline| {
                                    timeline.tracks.get(track_idx)
                                        .and_then(|track| track.instrument_id)
                                }).flatten().map(|inst_id| {
                                    let instruments = self.engine_state.instruments.lock().unwrap();
                                    instruments.get(&inst_id).map(|i| i.is_drum()).unwrap_or(false)
                                }).unwrap_or(false);

                                if is_drum {
                                    // Use drum roll panel for drum instruments
                                    let action = self.engine.with_timeline(|timeline| {
                                        if let Some(track) = timeline.tracks.get_mut(track_idx) {
                                            if let Some(clip) = track.midi_clips.iter_mut().find(|c| c.id == clip_id) {
                                                let clip_start = clip.start_sample;
                                                return Some(self.drum_roll_panel.ui(
                                                    ui, clip, bpm, sample_rate, clip_start, playback_position, &self.clipboard
                                                ));
                                            }
                                        }
                                        None
                                    }).flatten();

                                    if let Some(a) = action {
                                        drum_roll_action = a;
                                    }
                                } else {
                                    // Use piano roll panel for melodic instruments
                                    let action = self.engine.with_timeline(|timeline| {
                                        if let Some(track) = timeline.tracks.get_mut(track_idx) {
                                            if let Some(clip) = track.midi_clips.iter_mut().find(|c| c.id == clip_id) {
                                                let clip_start = clip.start_sample;
                                                return Some(self.clip_editor_panel.ui_midi(
                                                    ui, clip, bpm, sample_rate, clip_start, playback_position, &self.clipboard
                                                ));
                                            }
                                        }
                                        None
                                    }).flatten();

                                    if let Some(a) = action {
                                        piano_roll_action = a;
                                    }
                                }
                            }
                            SelectedClip::Audio { track_idx, clip_id } => {
                                let sample_rate = self.engine.sample_rate();

                                self.engine.with_timeline(|timeline| {
                                    if let Some(track) = timeline.tracks.get(track_idx) {
                                        if let Some(clip) = track.clips.iter().find(|c| c.id == clip_id) {
                                            self.clip_editor_panel.ui_audio(ui, clip, sample_rate);
                                        }
                                    }
                                });
                            }
                        }
                    }

                    // Handle piano roll actions
                    match piano_roll_action {
                        PianoRollAction::ClipModified => {
                            // Clip was modified - no additional action needed
                        }
                        PianoRollAction::SetLoopRegion { start_sample, end_sample } => {
                            self.engine.with_timeline(|timeline| {
                                timeline.transport.loop_start = start_sample;
                                timeline.transport.loop_end = end_sample;
                                timeline.transport.loop_enabled = true;
                            });
                            tracing::info!("Loop region set: {} - {}", start_sample, end_sample);
                        }
                        PianoRollAction::PlayNote { pitch, velocity } => {
                            // Preview note - send to instrument
                            if let Some(SelectedClip::Midi { track_idx, .. }) = self.selected_clip {
                                let inst_id = self.engine.with_timeline(|t| {
                                    t.tracks.get(track_idx).and_then(|track| track.instrument_id)
                                }).flatten();

                                if let Some(id) = inst_id {
                                    let mut instruments = self.engine_state.instruments.lock().unwrap();
                                    if let Some(inst) = instruments.get_mut(&id) {
                                        inst.queue_note_on(pitch, velocity, 0, 0);
                                    }
                                }
                            }
                        }
                        PianoRollAction::StopNote { pitch } => {
                            // Stop preview note
                            if let Some(SelectedClip::Midi { track_idx, .. }) = self.selected_clip {
                                let inst_id = self.engine.with_timeline(|t| {
                                    t.tracks.get(track_idx).and_then(|track| track.instrument_id)
                                }).flatten();

                                if let Some(id) = inst_id {
                                    let mut instruments = self.engine_state.instruments.lock().unwrap();
                                    if let Some(inst) = instruments.get_mut(&id) {
                                        inst.queue_note_off(pitch, 0, 0, 0);
                                    }
                                }
                            }
                        }
                        PianoRollAction::StopNotes { pitches } => {
                            // Stop multiple notes (when deleting during playback)
                            if let Some(SelectedClip::Midi { track_idx, .. }) = self.selected_clip {
                                let inst_id = self.engine.with_timeline(|t| {
                                    t.tracks.get(track_idx).and_then(|track| track.instrument_id)
                                }).flatten();

                                if let Some(id) = inst_id {
                                    let mut instruments = self.engine_state.instruments.lock().unwrap();
                                    if let Some(inst) = instruments.get_mut(&id) {
                                        for pitch in pitches {
                                            inst.queue_note_off(pitch, 0, 0, 0);
                                        }
                                    }
                                }
                            }
                        }
                        PianoRollAction::RecordNote { pitch, velocity } => {
                            // Record note during playback - add to clip at current position
                            if let Some(SelectedClip::Midi { track_idx, clip_id }) = self.selected_clip {
                                let position = self.engine.position();
                                self.engine.with_timeline(|timeline| {
                                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                                        if let Some(clip) = track.midi_clips.iter_mut().find(|c| c.id == clip_id) {
                                            let bpm = timeline.transport.bpm;
                                            let sample_rate = timeline.transport.sample_rate;
                                            let samples_per_beat = sample_rate as f64 * 60.0 / bpm;

                                            // Calculate position within clip
                                            let clip_pos = position.saturating_sub(clip.start_sample);
                                            let beat = clip_pos as f64 / samples_per_beat;
                                            let start_tick = (beat * clip.ppq as f64) as u64;

                                            let note = signum_core::MidiNote::new(
                                                pitch,
                                                velocity,
                                                start_tick,
                                                clip.ppq as u64, // Default 1 beat duration
                                            );
                                            clip.add_note(note);
                                        }
                                    }
                                });
                            }
                        }
                        PianoRollAction::None => {}
                    }

                    // Handle drum roll actions
                    match drum_roll_action {
                        DrumRollAction::TogglePlayback { clip_start_sample, clip_end_sample } => {
                            if self.engine.is_playing() {
                                self.engine.pause();
                                self.engine.seek(self.playback_start_position);
                            } else {
                                self.playback_start_position = self.engine.position();
                                let loop_enabled = self.engine.is_loop_enabled();
                                if !loop_enabled {
                                    self.engine.with_timeline(|timeline| {
                                        timeline.transport.loop_start = clip_start_sample;
                                        timeline.transport.loop_end = clip_end_sample;
                                        timeline.transport.loop_enabled = true;
                                    });
                                }
                                let (loop_start, loop_end) = self.engine.loop_region();
                                let current_pos = self.engine.position();
                                if current_pos < loop_start || current_pos >= loop_end {
                                    self.engine.seek(loop_start);
                                    self.playback_start_position = loop_start;
                                }
                                self.engine.play();
                            }
                        }
                        DrumRollAction::ClipModified => {}
                        DrumRollAction::PlayNote { pitch, velocity } => {
                            if let Some(SelectedClip::Midi { track_idx, .. }) = self.selected_clip {
                                let inst_id = self.engine.with_timeline(|t| {
                                    t.tracks.get(track_idx).and_then(|track| track.instrument_id)
                                }).flatten();
                                if let Some(id) = inst_id {
                                    let mut instruments = self.engine_state.instruments.lock().unwrap();
                                    if let Some(inst) = instruments.get_mut(&id) {
                                        inst.queue_note_on(pitch, velocity, 0, 0);
                                    }
                                }
                            }
                        }
                        DrumRollAction::StopNote { pitch } => {
                            if let Some(SelectedClip::Midi { track_idx, .. }) = self.selected_clip {
                                let inst_id = self.engine.with_timeline(|t| {
                                    t.tracks.get(track_idx).and_then(|track| track.instrument_id)
                                }).flatten();
                                if let Some(id) = inst_id {
                                    let mut instruments = self.engine_state.instruments.lock().unwrap();
                                    if let Some(inst) = instruments.get_mut(&id) {
                                        inst.queue_note_off(pitch, 0, 0, 0);
                                    }
                                }
                            }
                        }
                        DrumRollAction::SetLoopRegion { start_sample, end_sample } => {
                            self.engine.with_timeline(|timeline| {
                                timeline.transport.loop_start = start_sample;
                                timeline.transport.loop_end = end_sample;
                                timeline.transport.loop_enabled = true;
                            });
                            tracing::info!("Loop region set: {} - {}", start_sample, end_sample);
                        }
                        DrumRollAction::None => {}
                    }
                });
        }

        // 4. Left sidebar: Browser
        let plugins = self.get_plugins();
        egui::SidePanel::left("browser")
            .resizable(true)
            .default_width(180.0)
            .min_width(120.0)
            .show(ctx, |ui| {
                let action = self.browser_panel.ui(ui, &plugins);
                match action {
                    BrowserAction::LoadEffect(info) => self.load_vst3_effect(&info),
                    BrowserAction::LoadInstrument(info) => self.load_instrument_to_track(&info),
                    BrowserAction::LoadNativeInstrument(info) => self.load_native_drum(info.id),
                    BrowserAction::LoadSample(path) => self.load_sample(&path),
                    BrowserAction::AddPlace(path) => {
                        self.browser_panel.add_place(path);
                        self.save_library_config();
                    }
                    BrowserAction::RemovePlace(idx) => {
                        self.browser_panel.remove_place(idx);
                        self.save_library_config();
                    }
                    BrowserAction::SelectFile(path) => {
                        tracing::debug!("Browser SelectFile → clipboard = {:?}", path);
                        self.clipboard.copy(ClipboardContent::FilePath(path));
                    }
                    BrowserAction::None => {}
                }
            });

        // 5. Central panel: Track Headers + Arrange
        egui::CentralPanel::default().show(ctx, |ui| {
            let panel_height = ui.available_height();

            ui.horizontal(|ui| {
                // Track headers (fixed width)
                let tracks: Vec<_> = self.engine.with_timeline(|t| {
                    t.tracks.clone()
                }).unwrap_or_default();

                // Use fixed size for track headers column
                ui.allocate_ui_with_layout(
                    Vec2::new(120.0, panel_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let action = self.track_headers_panel.ui(ui, &tracks, self.selected_track_idx);
                        self.handle_track_header_action(action);
                    }
                );

                ui.separator();

                // Arrange timeline
                let recording_preview = if self.input_monitor.is_recording() {
                    self.input_monitor.get_recording_preview().map(|samples| {
                        RecordingPreview {
                            samples,
                            start_sample: self.recording_start_sample,
                            sample_rate: self.input_monitor.sample_rate(),
                        }
                    })
                } else {
                    None
                };

                let selected_clip_tuple = self.selected_clip.map(|c| match c {
                    SelectedClip::Audio { track_idx, clip_id } => (track_idx, clip_id),
                    SelectedClip::Midi { track_idx, clip_id } => (track_idx, clip_id),
                });

                let action = self.arrange_panel.ui(
                    ui,
                    &self.engine,
                    &self.engine_state,
                    self.selected_track_idx,
                    selected_clip_tuple,
                    recording_preview,
                    &self.clipboard,
                );
                self.handle_arrange_action(action);
            });
        });

        // 6. Floating plugin windows with parameter controls
        let mut param_updates: Vec<(u64, String, f32)> = Vec::new();
        let mut native_window_requests: Vec<(u64, String, String, String)> = Vec::new();

        self.plugin_windows.retain_mut(|window| {
            let mut still_open = window.open;

            // Get params from instrument (clone to release lock quickly)
            let params = {
                let instruments = self.engine_state.instruments.lock().unwrap();
                instruments.get(&window.id).map(|inst| inst.get_params().to_vec())
            };

            egui::Window::new(&window.title)
                .open(&mut still_open)
                .resizable(true)
                .default_size([350.0, 500.0])
                .show(ctx, |ui| {
                    let Some(params) = params else {
                        ui.label("Plugin not found");
                        return;
                    };

                    // Native GUI button
                    ui.horizontal(|ui| {
                        if !window.native_window_created {
                            if ui.button("Open Native GUI").clicked() {
                                native_window_requests.push((
                                    window.id,
                                    window.plugin_path.clone(),
                                    window.plugin_uid.clone(),
                                    window.title.clone(),
                                ));
                            }
                        } else {
                            ui.label("Native GUI active");
                            if ui.button("Show").clicked() {
                                let _ = self.gui_manager.show_window(window.id);
                            }
                        }
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label(format!("{} parameters", params.len()));
                    });
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for param in params {
                            ui.horizontal(|ui| {
                                ui.label(&param.name);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if !param.unit.is_empty() {
                                        ui.label(&param.unit);
                                    }
                                });
                            });

                            let mut value = param.value;
                            let range = param.min..=param.max;
                            let slider = egui::Slider::new(&mut value, range)
                                .show_value(true)
                                .clamping(egui::SliderClamping::Always);

                            if ui.add(slider).changed() {
                                param_updates.push((window.id, param.name.clone(), value));
                            }
                            ui.add_space(4.0);
                        }
                    });
                });
            window.open = still_open;
            still_open
        });

        // 7. Native instrument parameter windows (808, etc.)
        let mut windows_to_close: Vec<u64> = Vec::new();
        for &inst_id in &self.native_param_windows {
            let mut still_open = true;

            // Get instrument name and params
            let (name, params) = {
                let instruments = self.engine_state.instruments.lock().unwrap();
                instruments.get(&inst_id).map(|inst| {
                    (inst.name().to_string(), inst.get_params().to_vec())
                }).unwrap_or_else(|| ("Unknown".to_string(), Vec::new()))
            };

            egui::Window::new(&name)
                .id(egui::Id::new(format!("native_param_{}", inst_id)))
                .open(&mut still_open)
                .resizable(true)
                .default_size([300.0, 400.0])
                .show(ctx, |ui| {
                    ui.label(format!("{} parameters", params.len()));
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for param in &params {
                            ui.horizontal(|ui| {
                                ui.label(&param.name);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if !param.unit.is_empty() {
                                        ui.label(&param.unit);
                                    }
                                });
                            });

                            let mut value = param.value;
                            let range = param.min..=param.max;
                            let slider = egui::Slider::new(&mut value, range)
                                .show_value(true)
                                .clamping(egui::SliderClamping::Always);

                            if ui.add(slider).changed() {
                                param_updates.push((inst_id, param.name.clone(), value));
                            }
                            ui.add_space(4.0);
                        }
                    });
                });

            if !still_open {
                windows_to_close.push(inst_id);
            }
        }
        for id in windows_to_close {
            self.native_param_windows.remove(&id);
        }

        // Create native windows for requested plugins
        for (id, path, uid, title) in native_window_requests {
            if let Err(e) = self.gui_manager.create_window(id, &path, &uid, &title, 800, 600) {
                tracing::warn!("Failed to create native window for plugin {}: {}", id, e);
            } else {
                if let Err(e) = self.gui_manager.show_window(id) {
                    tracing::warn!("Failed to show native window: {}", e);
                }
                // Mark window as having native GUI
                if let Some(window) = self.plugin_windows.iter_mut().find(|w| w.id == id) {
                    window.native_window_created = true;
                }
                tracing::info!("Created native GUI window for plugin {}", id);
            }
        }

        // Apply parameter updates outside of UI loop
        if !param_updates.is_empty() {
            let mut instruments = self.engine_state.instruments.lock().unwrap();
            for (id, name, value) in param_updates {
                if let Some(inst) = instruments.get_mut(&id) {
                    inst.set_param(&name, value);
                }
            }
        }

        // Process native window events
        let _ = self.gui_manager.process_events();

        // NOTE: Full state/preset syncing from native plugin GUIs is disabled due to
        // thread-safety issues with VST3 plugins. The GUI and audio are separate plugin
        // instances. Parameter tweaks are synced via get_parameter_changes() below, but
        // preset changes in the plugin's native UI won't affect audio playback.
        // TODO: Consider sharing a single plugin instance between GUI and audio.
        let _state_changes = self.gui_manager.get_state_changes();
        if !_state_changes.is_empty() {
            tracing::warn!(
                "Preset changed in native plugin GUI - this won't affect audio playback. \
                 Use the DAW's preset management instead."
            );
        }

        // Sync parameter changes from native plugin GUIs to audio instruments
        let param_changes = self.gui_manager.get_parameter_changes();
        if !param_changes.is_empty() {
            tracing::info!("Syncing {} parameter changes from GUI", param_changes.len());
            let mut instruments = self.engine_state.instruments.lock().unwrap();
            let inst_ids: Vec<_> = instruments.keys().copied().collect();
            tracing::info!("Available instrument IDs: {:?}", inst_ids);
            for (plugin_id, param_index, value) in param_changes {
                tracing::info!("Looking for plugin_id={} to set param[{}]={}", plugin_id, param_index, value);
                if let Some(inst) = instruments.get_mut(&plugin_id) {
                    tracing::info!("Found instrument, setting param {} = {}", param_index, value);
                    inst.set_param_by_index(param_index, value);
                } else {
                    tracing::warn!("Plugin {} not found in instruments (available: {:?})", plugin_id, inst_ids);
                }
            }
        }

        // Request repaint for animation
        if self.engine.is_playing() || self.input_monitor.is_running() {
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.input_monitor.stop();
        let _ = self.engine.stop();
    }
}
