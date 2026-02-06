//! Main application state

mod action_handlers;
mod audio_ops;
mod config;
mod plugin_windows;
mod sample_kit_ops;
mod track_ops;
mod types;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::CreationContext;
use egui::{Context, Vec2};
use hallucinator_core::{PlaybackMode, SongSection, TrackKind};
use hallucinator_services::{
    AudioEngine, EngineState, InputMonitor, MeterState, PluginGuiManager,
};

pub use types::SelectedClip;
use config::load_config;
use types::PluginWindow;

use crate::clipboard::DawClipboard;
use crate::panels::{
    ArrangePanel, BrowserPanel, ClipEditorPanel,
    DeviceRackAction, DeviceRackPanel, DrumRollAction, DrumRollPanel,
    KeyboardSequencerPanel,
    MidiFxRackPanel,
    PianoRollAction, PluginBrowserPanel,
    RecordingPreview, SongViewPanel,
    TrackHeadersPanel, TransportAction, TransportPanel,
};


pub struct HallucinatorApp {
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
    keyboard_sequencer_panel: KeyboardSequencerPanel,
    midi_fx_rack_panel: MidiFxRackPanel,
    song_view_panel: SongViewPanel,

    // App-wide clipboard
    clipboard: DawClipboard,

    // Factory Rat panel visibility
    show_factory_rat_panels: bool,

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

    // Recording state
    recording_start_sample: u64,

    // Playback start position (for space toggle return-to-start)
    playback_start_position: u64,
}

impl HallucinatorApp {
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
            keyboard_sequencer_panel: KeyboardSequencerPanel::new(),
            midi_fx_rack_panel: MidiFxRackPanel::new(),
            song_view_panel: SongViewPanel::new(),
            clipboard: DawClipboard::default(),
            show_factory_rat_panels: true,
            selected_track_idx: Some(0),
            selected_clip: None,
            show_clip_editor: false,
            plugin_windows: Vec::new(),
            native_param_windows: HashSet::new(),
            gui_manager,
            next_clip_id: 1,
            next_instrument_id: 1,
            recording_start_sample: 0,
            playback_start_position: 0,
        }
    }



}

impl eframe::App for HallucinatorApp {
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

        // Consume Tab globally to prevent egui's focus navigation from stealing it
        // Check if shift is held, then consume Tab with any modifiers
        let shift_held = ctx.input(|i| i.modifiers.shift);
        let tab_pressed = ctx.input(|i| i.key_pressed(egui::Key::Tab));
        if tab_pressed {
            // Consume both variants to fully remove Tab from the event queue
            ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
            });
            self.keyboard_sequencer_panel.set_pending_tab(shift_held);
        }

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

        // Global Delete → delete selected clip (if any)
        if !text_focused && ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            if let Some(selected) = self.selected_clip.take() {
                self.delete_selected_clip(selected);
            }
        }

        // 1. Menu bar
        let plugin_action = egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.plugin_menu.menu_ui(ui, &mut self.arrange_panel.snap_to_grid)
        }).inner;

        self.handle_plugin_action(plugin_action);

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

        // 3b. Factory Rat Sequencer panel (above device rack)
        if self.show_factory_rat_panels {
            egui::TopBottomPanel::bottom("factory_rat_panel")
                .resizable(true)
                .default_height(160.0)
                .min_height(100.0)
                .show(ctx, |ui| {
                    // Get selected track info
                    let (track_name, midi_fx_chain) = self.selected_track_idx
                        .and_then(|idx| {
                            self.engine.with_timeline(|timeline| {
                                timeline.tracks.get(idx).map(|track| {
                                    (
                                        Some(track.name.clone()),
                                        Some(track.midi_fx_chain.clone()),
                                    )
                                })
                            }).flatten()
                        })
                        .unwrap_or((None, None));

                    ui.horizontal(|ui| {
                        ui.heading("Sequencer");
                        if let Some(name) = &track_name {
                            ui.separator();
                            ui.label(name);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Hide").clicked() {
                                self.show_factory_rat_panels = false;
                            }
                            if ui.small_button("?").on_hover_text("Open docs/sequencer.md").clicked() {
                                let _ = open::that("docs/sequencer.md");
                            }
                        });
                    });
                    ui.separator();

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
                                    is_playing,
                                    &self.clipboard,
                                    &self.engine_state,
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
            egui::TopBottomPanel::bottom("factory_rat_toggle")
                .resizable(false)
                .exact_height(20.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("▲ Show Sequencer").clicked() {
                            self.show_factory_rat_panels = true;
                        }
                    });
                });
        }

        // 3c. Floating keyboard sequencer window
        if self.keyboard_sequencer_panel.is_floating {
            let is_playing = self.engine.is_playing();
            let track_name: Option<String> = self.selected_track_idx.and_then(|idx| {
                self.engine.with_timeline(|t| t.tracks.get(idx).map(|tr| tr.name.clone())).flatten()
            });

            let mut still_open = true;
            egui::Window::new("FACTORY RAT Sequencer")
                .open(&mut still_open)
                .resizable(true)
                .default_size([1020.0, 460.0])
                .show(ctx, |ui| {
                    let actions = self.keyboard_sequencer_panel.ui(
                        ui,
                        track_name.as_deref(),
                        is_playing,
                        &self.clipboard,
                        &self.engine_state,
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
                                }).flatten().and_then(|inst_id| {
                                    let instruments = self.engine_state.instruments.lock().ok()?;
                                    Some(instruments.get(&inst_id)?.is_drum())
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

                    self.handle_piano_roll_action(piano_roll_action);
                    self.handle_drum_roll_action(drum_roll_action);
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
                self.handle_browser_action(action);
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
            let params = self.engine_state.instruments.lock().ok()
                .and_then(|instruments| instruments.get(&window.id).map(|inst| inst.get_params().to_vec()));

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
                        for (name, value) in plugin_windows::render_param_sliders(ui, &params) {
                            param_updates.push((window.id, name, value));
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
            let (name, params) = self.engine_state.instruments.lock().ok()
                .and_then(|instruments| instruments.get(&inst_id).map(|inst| {
                    (inst.name().to_string(), inst.get_params().to_vec())
                }))
                .unwrap_or_else(|| ("Unknown".to_string(), Vec::new()));

            egui::Window::new(&name)
                .id(egui::Id::new(format!("native_param_{}", inst_id)))
                .open(&mut still_open)
                .resizable(true)
                .default_size([300.0, 400.0])
                .show(ctx, |ui| {
                    ui.label(format!("{} parameters", params.len()));
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (name, value) in plugin_windows::render_param_sliders(ui, &params) {
                            param_updates.push((inst_id, name, value));
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
                continue;
            }
            if let Err(e) = self.gui_manager.show_window(id) {
                tracing::warn!("Failed to show native window: {}", e);
            }
            // Mark window as having native GUI
            if let Some(window) = self.plugin_windows.iter_mut().find(|w| w.id == id) {
                window.native_window_created = true;
            }
            tracing::info!("Created native GUI window for plugin {}", id);
        }

        // Apply parameter updates outside of UI loop
        if !param_updates.is_empty() {
            if let Ok(mut instruments) = self.engine_state.instruments.lock() {
                for (id, name, value) in param_updates {
                    if let Some(inst) = instruments.get_mut(&id) {
                        inst.set_param(&name, value);
                    }
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
            if let Ok(mut instruments) = self.engine_state.instruments.lock() {
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
