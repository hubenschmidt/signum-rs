use hallucinator_core::ClipId;
use hallucinator_services::{Instrument, Vst3PluginInfo};

use super::HallucinatorApp;
use super::types::SelectedClip;
use crate::panels::{
    ArrangeAction, BrowserAction, DeviceInfo, DeviceRackAction, DrumRollAction,
    KeyboardSequencerAction, MidiFxRackAction, PianoRollAction, PluginAction, SongViewAction,
    TrackHeaderAction,
};

impl HallucinatorApp {
    pub(super) fn handle_track_header_action(&mut self, action: TrackHeaderAction) {
        match action {
            TrackHeaderAction::SelectTrack(idx) => {
                self.selected_track_idx = Some(idx);
            }
            TrackHeaderAction::ToggleMute(idx) => {
                self.with_track_mut(idx, |track| track.mute = !track.mute);
            }
            TrackHeaderAction::ToggleSolo(idx) => {
                self.with_track_mut(idx, |track| track.solo = !track.solo);
            }
            TrackHeaderAction::ToggleArm(idx) => {
                self.with_track_mut(idx, |track| track.armed = !track.armed);
            }
            TrackHeaderAction::SetVolume(idx, vol) => {
                self.with_track_mut(idx, |track| track.volume = vol);
            }
            TrackHeaderAction::SetPan(idx, pan) => {
                self.with_track_mut(idx, |track| track.pan = pan);
            }
            TrackHeaderAction::DeleteTrack(idx) => {
                self.engine.with_timeline(|timeline| {
                    if idx < timeline.tracks.len() {
                        timeline.tracks.remove(idx);
                    }
                });
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
            TrackHeaderAction::RenameTrack(idx, name) => {
                self.with_track_mut(idx, |track| track.name = name);
            }
            TrackHeaderAction::None => {}
        }
    }

    pub(super) fn handle_arrange_action(&mut self, action: ArrangeAction) {
        match action {
            ArrangeAction::SelectClip { track_idx, clip_id } => {
                self.selected_track_idx = Some(track_idx);
                self.selected_clip = self.resolve_clip_type(track_idx, clip_id);
            }
            ArrangeAction::OpenClipEditor { track_idx, clip_id } => {
                self.selected_track_idx = Some(track_idx);
                self.selected_clip = self.resolve_clip_type(track_idx, clip_id);
                self.show_clip_editor = true;
            }
            ArrangeAction::DeleteClip { track_idx, clip_id } => {
                if let Some(clip) = self.resolve_clip_type(track_idx, clip_id) {
                    self.delete_selected_clip(clip);
                }
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
            ArrangeAction::SetLoopRegion {
                start_sample,
                end_sample,
            } => {
                self.engine.set_loop_region(start_sample, end_sample);
                self.engine.set_loop_enabled(true);
            }
            ArrangeAction::None => {}
        }
    }

    pub(super) fn handle_device_rack_action(&mut self, action: DeviceRackAction) {
        match action {
            DeviceRackAction::OpenPluginWindow(id) => {
                tracing::info!("OpenPluginWindow action for id={}", id);

                if self.gui_manager.has_window(id) {
                    if let Err(e) = self.gui_manager.show_window(id) {
                        tracing::warn!("Failed to show plugin window: {}", e);
                    }
                    return;
                }

                let Ok(instruments) = self.engine_state.instruments.lock() else {
                    return;
                };
                let Some(inst) = instruments.get(&id) else {
                    return;
                };

                // Check for VST3 plugin info first
                if let Some(info) = inst.vst3_plugin_info() {
                    let (title, plugin_path, plugin_uid) = (
                        info.name.clone(),
                        info.info.path.to_string_lossy().to_string(),
                        info.info.unique_id.clone(),
                    );
                    drop(instruments);
                    self.open_native_plugin_gui(id, &plugin_path, &plugin_uid, &title);
                    return;
                }

                // Handle native instrument param window toggle
                let is_native = inst.is_drum();
                tracing::info!("Checking if id={} is native: {}", id, is_native);
                drop(instruments);

                if !is_native {
                    return;
                }

                let action = if self.native_param_windows.contains(&id) {
                    "Closing"
                } else {
                    "Opening"
                };
                tracing::info!("{} native param window for id={}", action, id);

                if self.native_param_windows.contains(&id) {
                    self.native_param_windows.remove(&id);
                } else {
                    self.native_param_windows.insert(id);
                }
            }
            DeviceRackAction::ToggleBypass(_id) => {}
            DeviceRackAction::RemoveDevice(_id) => {}
            DeviceRackAction::AddEffect => {}
            DeviceRackAction::None => {}
        }
    }

    pub(super) fn handle_plugin_action(&mut self, action: PluginAction) {
        match action {
            PluginAction::LoadPlugin(info) => self.load_vst3_effect(&info),
            PluginAction::CreateMidiTrack(info) => self.load_instrument_to_track(&info),
            PluginAction::AddAudioTrack => self.add_audio_track(),
            PluginAction::AddMidiTrack => self.add_empty_midi_track(),
            PluginAction::None => {}
        }
    }

    pub(super) fn handle_browser_action(&mut self, action: BrowserAction) {
        match action {
            BrowserAction::LoadEffect(info) => self.load_vst3_effect(&info),
            BrowserAction::LoadInstrument(info) => self.load_instrument_to_track(&info),
            BrowserAction::LoadNativeInstrument(info) => self.load_native_drum(info.id),
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
                self.clipboard
                    .copy(crate::clipboard::ClipboardContent::FilePath(path));
            }
            BrowserAction::None => {}
        }
    }

    pub(super) fn handle_keyboard_sequencer_actions(
        &mut self,
        actions: Vec<KeyboardSequencerAction>,
    ) {
        let Some(track_idx) = self.selected_track_idx else {
            return;
        };

        for action in actions {
            match action {
                KeyboardSequencerAction::ToggleDrumStep(step) => {
                    tracing::debug!("Toggle drum step {}", step);
                }
                KeyboardSequencerAction::PlayNote { pitch, velocity } => {
                    self.send_note_on(track_idx, pitch, velocity);
                }
                KeyboardSequencerAction::StopNote { pitch } => {
                    self.send_note_off(track_idx, pitch);
                }
                KeyboardSequencerAction::LoadStepSample { step, layer, path } => {
                    tracing::debug!(
                        "LoadStepSample step={} layer={} path={:?}",
                        step,
                        layer,
                        path
                    );
                    self.load_step_sample(track_idx, step, layer, &path);
                }
                KeyboardSequencerAction::PlayDrumStep {
                    step,
                    velocity,
                    active_layers,
                } => {
                    let inst_id = self
                        .engine
                        .with_timeline(|t| {
                            t.tracks
                                .get(track_idx)
                                .and_then(|track| track.instrument_id)
                        })
                        .flatten();
                    let Some(id) = inst_id else { continue };
                    let Ok(mut instruments) = self.engine_state.instruments.lock() else {
                        continue;
                    };
                    let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&id) else {
                        continue;
                    };
                    kit.trigger_step(step, velocity, active_layers);
                }
                KeyboardSequencerAction::CopyStepSample {
                    from_step,
                    from_layer,
                    to_step,
                    to_layer,
                } => {
                    self.copy_step_sample(track_idx, from_step, from_layer, to_step, to_layer);
                }
                KeyboardSequencerAction::MoveStepSample {
                    from_step,
                    from_layer,
                    to_step,
                    to_layer,
                } => {
                    self.copy_step_sample(track_idx, from_step, from_layer, to_step, to_layer);
                    self.clear_step_sample(track_idx, from_step, from_layer);
                    self.keyboard_sequencer_panel.clear_step_sample_name(from_step, from_layer);
                }
                KeyboardSequencerAction::CopyDrumStep { step, layer } => {
                    tracing::debug!("CopyDrumStep step={} layer={}", step, layer);
                    self.copy_drum_step_to_clipboard(track_idx, step, layer);
                }
                KeyboardSequencerAction::PasteStepSample {
                    step,
                    layer,
                    name,
                    data,
                } => {
                    tracing::debug!(
                        "PasteStepSample step={} layer={} name={}",
                        step,
                        layer,
                        name
                    );
                    self.paste_step_sample(track_idx, step, layer, name, data);
                }
                KeyboardSequencerAction::ClearStepSample { step, layer } => {
                    self.clear_step_sample(track_idx, step, layer);
                }
            }
        }
    }

    pub(super) fn handle_midi_fx_rack_action(&mut self, action: MidiFxRackAction) {
        let Some(track_idx) = self.selected_track_idx else {
            return;
        };

        match action {
            MidiFxRackAction::AddEffect(effect_type) => {
                let effect = effect_type.create_effect();
                self.with_track_mut(track_idx, |track| track.midi_fx_chain.add(effect));
            }
            MidiFxRackAction::RemoveEffect(idx) => {
                self.with_track_mut(track_idx, |track| {
                    track.midi_fx_chain.remove(idx);
                });
            }
            MidiFxRackAction::ToggleBypass(idx) => {
                self.with_track_mut(track_idx, |track| {
                    if let Some(effect) = track.midi_fx_chain.effects.get_mut(idx) {
                        effect.set_bypass(!effect.is_bypassed());
                    }
                });
            }
            MidiFxRackAction::MoveEffect { from, to } => {
                self.with_track_mut(track_idx, |track| {
                    let effects = &mut track.midi_fx_chain.effects;
                    if from < effects.len() && to < effects.len() {
                        let effect = effects.remove(from);
                        effects.insert(to, effect);
                    }
                });
            }
            MidiFxRackAction::SetParam {
                effect_idx,
                param_name,
                value,
            } => {
                self.with_track_mut(track_idx, |track| {
                    if let Some(effect) = track.midi_fx_chain.effects.get_mut(effect_idx) {
                        effect.set_param(&param_name, value);
                    }
                });
            }
            MidiFxRackAction::None => {}
        }
    }

    pub(super) fn handle_song_view_action(&mut self, action: SongViewAction) {
        match action {
            SongViewAction::SelectSection(_idx) => {}
            SongViewAction::AddSection => {}
            SongViewAction::RemoveSection(_idx) => {}
            SongViewAction::DuplicateSection(_idx) => {}
            SongViewAction::SetPlaybackMode(_mode) => {}
            SongViewAction::JumpToSection(_idx) => {}
            SongViewAction::MoveSection { from: _, to: _ } => {}
            SongViewAction::SetSectionLength { index: _, bars: _ } => {}
            SongViewAction::SetSectionRepeat { index: _, count: _ } => {}
            SongViewAction::None => {}
        }
    }

    pub(super) fn handle_piano_roll_action(&mut self, action: PianoRollAction) {
        let Some(SelectedClip::Midi { track_idx, .. }) = self.selected_clip else {
            return;
        };

        match action {
            PianoRollAction::PlayNote { pitch, velocity } => {
                self.send_note_on(track_idx, pitch, velocity);
            }
            PianoRollAction::StopNote { pitch } => {
                self.send_note_off(track_idx, pitch);
            }
            PianoRollAction::StopNotes { pitches } => {
                for pitch in pitches {
                    self.send_note_off(track_idx, pitch);
                }
            }
            PianoRollAction::SetLoopRegion {
                start_sample,
                end_sample,
            } => {
                self.engine.set_loop_region(start_sample, end_sample);
                self.engine.set_loop_enabled(true);
            }
            PianoRollAction::RecordNote { .. } => {}
            PianoRollAction::ClipModified | PianoRollAction::None => {}
        }
    }

    pub(super) fn handle_drum_roll_action(&mut self, action: DrumRollAction) {
        let Some(SelectedClip::Midi { track_idx, .. }) = self.selected_clip else {
            return;
        };

        match action {
            DrumRollAction::PlayNote { pitch, velocity } => {
                self.send_note_on(track_idx, pitch, velocity);
            }
            DrumRollAction::TogglePlayback { clip_start_sample } => {
                if self.engine.is_playing() {
                    self.engine.pause();
                } else {
                    self.engine.seek(clip_start_sample);
                    self.engine.play();
                }
            }
            DrumRollAction::SetLoopRegion {
                start_sample,
                end_sample,
            } => {
                self.engine.set_loop_region(start_sample, end_sample);
                self.engine.set_loop_enabled(true);
            }
            DrumRollAction::ClipModified | DrumRollAction::None => {}
        }
    }

    // ── Shared helpers ──

    pub(super) fn get_device_info_for_track(
        &self,
        track_idx: usize,
    ) -> (Option<DeviceInfo>, Vec<DeviceInfo>) {
        let inst_id = self
            .engine
            .with_timeline(|timeline| {
                timeline
                    .tracks
                    .get(track_idx)
                    .and_then(|track| track.instrument_id)
            })
            .flatten();

        let instrument = inst_id.and_then(|id| {
            let instruments = self.engine_state.instruments.lock().ok()?;
            let name = instruments
                .get(&id)
                .map(|i| i.name().to_string())
                .unwrap_or_else(|| "Instrument".to_string());
            Some(DeviceInfo {
                id,
                name,
                is_instrument: true,
                is_bypassed: false,
                has_ui: true,
            })
        });

        let effects = Vec::new();
        (instrument, effects)
    }

    pub(super) fn get_plugins(&self) -> Vec<Vst3PluginInfo> {
        self.plugin_menu
            .scanner()
            .map(|s| s.plugins().to_vec())
            .unwrap_or_default()
    }

    /// Determine whether a clip is audio or MIDI.
    fn resolve_clip_type(&self, track_idx: usize, clip_id: ClipId) -> Option<SelectedClip> {
        let clip_type = self
            .engine
            .with_timeline(|timeline| {
                let track = timeline.tracks.get(track_idx)?;
                if track.clips.iter().any(|c| c.id == clip_id) {
                    return Some(false);
                }
                if track.midi_clips.iter().any(|c| c.id == clip_id) {
                    return Some(true);
                }
                None
            })
            .flatten();

        clip_type.map(|is_midi| {
            if is_midi {
                SelectedClip::Midi { track_idx, clip_id }
            } else {
                SelectedClip::Audio { track_idx, clip_id }
            }
        })
    }

    /// Send note-on to the instrument on the given track.
    pub(super) fn send_note_on(&self, track_idx: usize, pitch: u8, velocity: u8) {
        let inst_id = self
            .engine
            .with_timeline(|t| {
                t.tracks
                    .get(track_idx)
                    .and_then(|track| track.instrument_id)
            })
            .flatten();
        let Some(id) = inst_id else { return };
        let Ok(mut instruments) = self.engine_state.instruments.lock() else {
            return;
        };
        let Some(inst) = instruments.get_mut(&id) else {
            return;
        };
        inst.queue_note_on(pitch, velocity, 0, 0);
    }

    /// Send note-off to the instrument on the given track.
    pub(super) fn send_note_off(&self, track_idx: usize, pitch: u8) {
        let inst_id = self
            .engine
            .with_timeline(|t| {
                t.tracks
                    .get(track_idx)
                    .and_then(|track| track.instrument_id)
            })
            .flatten();
        let Some(id) = inst_id else { return };
        let Ok(mut instruments) = self.engine_state.instruments.lock() else {
            return;
        };
        let Some(inst) = instruments.get_mut(&id) else {
            return;
        };
        inst.queue_note_off(pitch, 0, 0, 0);
    }
}
