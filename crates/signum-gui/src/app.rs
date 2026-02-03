//! Main application state

use std::sync::Arc;

use eframe::CreationContext;
use egui::{Context, Vec2};
use signum_core::{AudioClip, ClipId, MidiClip, TrackKind};
use signum_services::{
    AudioEngine, EngineState, InputMonitor, MeterState, PluginGuiManager, Vst3Effect,
    Vst3Instrument, Vst3PluginInfo,
};

use crate::panels::{
    ArrangeAction, ArrangePanel, BrowserAction, BrowserPanel, ClipEditorPanel,
    DeviceInfo, DeviceRackAction, DeviceRackPanel, PianoRollAction, PluginAction, PluginBrowserPanel,
    RecordingPreview, TrackHeaderAction, TrackHeadersPanel, TransportAction, TransportPanel,
};

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

    // Selection state
    selected_track_idx: Option<usize>,
    selected_clip: Option<SelectedClip>,
    show_clip_editor: bool,

    // Floating windows
    plugin_windows: Vec<PluginWindow>,
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

impl SignumApp {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        let sample_rate = 44100;
        let mut engine = AudioEngine::new(sample_rate);
        let engine_state = engine.state();

        if let Err(e) = engine.start() {
            tracing::error!("Failed to start audio engine: {}", e);
        }

        // Create a default track
        engine.with_timeline(|timeline| {
            timeline.add_track(TrackKind::Audio, "Track 1");
        });

        let input_monitor = InputMonitor::new();
        let meter_state = input_monitor.meter_state();

        // Initialize native GUI manager for plugin windows
        let mut gui_manager = PluginGuiManager::new();
        if let Err(e) = gui_manager.initialize() {
            tracing::warn!("Failed to initialize native GUI manager: {}", e);
        }

        Self {
            engine,
            engine_state,
            input_monitor,
            meter_state,
            transport_panel: TransportPanel::new(),
            plugin_menu: PluginBrowserPanel::new(),
            browser_panel: BrowserPanel::new(),
            track_headers_panel: TrackHeadersPanel::new(),
            arrange_panel: ArrangePanel::new(),
            device_rack_panel: DeviceRackPanel::new(),
            clip_editor_panel: ClipEditorPanel::new(),
            selected_track_idx: Some(0),
            selected_clip: None,
            show_clip_editor: false,
            plugin_windows: Vec::new(),
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
        self.engine.add_instrument(inst_id, instrument);

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
            ArrangeAction::TogglePlayback => {
                if self.engine.is_playing() {
                    // Stop and return to start position
                    self.engine.pause();
                    self.engine.seek(self.playback_start_position);
                } else {
                    // Save current position and start playing
                    self.playback_start_position = self.engine.position();
                    self.engine.play();
                }
            }
            ArrangeAction::None => {}
        }
    }

    fn handle_device_rack_action(&mut self, action: DeviceRackAction) {
        match action {
            DeviceRackAction::OpenPluginWindow(id) => {
                // Check if native window already exists
                if self.gui_manager.has_window(id) {
                    // Just show the existing window
                    if let Err(e) = self.gui_manager.show_window(id) {
                        tracing::warn!("Failed to show plugin window: {}", e);
                    }
                    return;
                }

                // Get plugin info from instruments
                let plugin_info = {
                    let instruments = self.engine_state.instruments.lock().unwrap();
                    instruments.get(&id).map(|inst| {
                        let info = inst.plugin_info();
                        (
                            info.name.clone(),
                            info.info.path.to_string_lossy().to_string(),
                            info.info.unique_id.clone(),
                        )
                    })
                };

                let Some((title, plugin_path, plugin_uid)) = plugin_info else {
                    tracing::warn!("Plugin {} not found", id);
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
        let instrument = self.engine.with_timeline(|timeline| {
            timeline.tracks.get(track_idx).and_then(|track| {
                track.instrument_id.map(|id| DeviceInfo {
                    id,
                    name: "Instrument".to_string(),
                    is_instrument: true,
                    is_bypassed: false,
                    has_ui: true,
                })
            })
        }).flatten();

        // TODO: Get actual effect chain from track
        let effects = Vec::new();

        (instrument, effects)
    }

    fn get_plugins(&self) -> Vec<Vst3PluginInfo> {
        self.plugin_menu.scanner()
            .map(|s| s.plugins().to_vec())
            .unwrap_or_default()
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

        // 3. Bottom panel: Device Rack (always visible, shorter)
        egui::TopBottomPanel::bottom("device_rack_panel")
            .resizable(false)
            .exact_height(60.0)
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
                        instrument,
                        &effects,
                    );
                    self.handle_device_rack_action(action);
                });
            });

        // 4. Clip Editor / Piano Roll panel (above device rack, only when clip selected)
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

                    if let Some(selected) = self.selected_clip {
                        match selected {
                            SelectedClip::Midi { track_idx, clip_id } => {
                                let (bpm, sample_rate) = self.engine.with_timeline(|t| {
                                    (t.transport.bpm, t.transport.sample_rate)
                                }).unwrap_or((120.0, 44100));

                                let playback_position = self.engine.position();

                                let action = self.engine.with_timeline(|timeline| {
                                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                                        if let Some(clip) = track.midi_clips.iter_mut().find(|c| c.id == clip_id) {
                                            let clip_start = clip.start_sample;
                                            return Some(self.clip_editor_panel.ui_midi(
                                                ui, clip, bpm, sample_rate, clip_start, playback_position
                                            ));
                                        }
                                    }
                                    None
                                }).flatten();

                                if let Some(a) = action {
                                    piano_roll_action = a;
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
                        PianoRollAction::TogglePlayback { clip_start_sample, clip_end_sample } => {
                            if self.engine.is_playing() {
                                // Stop and return to start position, disable clip loop
                                self.engine.pause();
                                self.engine.seek(self.playback_start_position);
                                self.engine.with_timeline(|timeline| {
                                    timeline.transport.loop_enabled = false;
                                });
                            } else {
                                // Save position, set loop to clip bounds, start playing
                                self.playback_start_position = self.engine.position();

                                // If position is outside clip, seek to clip start
                                let current_pos = self.engine.position();
                                if current_pos < clip_start_sample || current_pos >= clip_end_sample {
                                    self.engine.seek(clip_start_sample);
                                    self.playback_start_position = clip_start_sample;
                                }

                                // Set loop to clip boundaries
                                self.engine.with_timeline(|timeline| {
                                    timeline.transport.loop_start = clip_start_sample;
                                    timeline.transport.loop_end = clip_end_sample;
                                    timeline.transport.loop_enabled = true;
                                });

                                self.engine.play();
                            }
                        }
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
                instruments.get(&window.id).map(|inst| inst.get_params())
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

        // Sync component state changes (preset/patch loads) from native plugin GUIs
        // TODO: State sync is currently disabled as it may cause audio issues
        // Need to investigate proper VST3 state synchronization approach
        // let state_changes = self.gui_manager.get_state_changes();
        // if !state_changes.is_empty() {
        //     let mut instruments = self.engine_state.instruments.lock().unwrap();
        //     for (plugin_id, state) in state_changes {
        //         if let Some(inst) = instruments.get_mut(&plugin_id) {
        //             if let Err(e) = inst.set_state(&state) {
        //                 tracing::warn!("Failed to sync plugin state: {}", e);
        //             }
        //         }
        //     }
        // }

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
