use hallucinator_core::{ClipId, MidiClip, TrackKind};
use hallucinator_services::{Drum808, Instrument, Vst3Effect, Vst3Instrument, Vst3PluginInfo};

use super::types::SelectedClip;
use super::HallucinatorApp;

impl HallucinatorApp {
    /// Helper to mutate a track by index within the timeline.
    pub(super) fn with_track_mut<F>(&self, track_idx: usize, f: F)
    where
        F: FnOnce(&mut hallucinator_core::Track),
    {
        self.engine.with_timeline(|timeline| {
            if let Some(track) = timeline.tracks.get_mut(track_idx) {
                f(track);
            }
        });
    }

    pub(super) fn add_audio_track(&mut self) {
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

    pub(super) fn add_empty_midi_track(&mut self) {
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

    /// Finds selected MIDI track or creates a new one, assigns instrument,
    /// and ensures it has a MIDI clip. Returns the track index.
    pub(super) fn ensure_midi_track(
        &mut self,
        inst_id: u64,
        track_name: &str,
        clip_name: &str,
        clip_id: u64,
    ) -> Option<usize> {
        let selected_midi_track = self.selected_track_idx.and_then(|idx| {
            self.engine.with_timeline(|timeline| {
                timeline.tracks.get(idx).and_then(|t| {
                    if t.kind == TrackKind::Midi { Some(idx) } else { None }
                })
            }).flatten()
        });

        let full_name = track_name.to_string();
        let clip_label = clip_name.to_string();

        if let Some(idx) = selected_midi_track {
            self.engine.with_timeline(|timeline| {
                let samples_per_beat = track_time_params(timeline);
                let Some(track) = timeline.tracks.get_mut(idx) else { return };
                track.instrument_id = Some(inst_id);
                track.name = full_name.clone();

                if track.midi_clips.is_empty() {
                    let length_samples = (samples_per_beat * 16.0) as u64;
                    let mut clip = MidiClip::new(ClipId(clip_id), length_samples);
                    clip.name = clip_label.clone();
                    track.add_midi_clip(clip);
                }
            });
            return Some(idx);
        }

        // Create new MIDI track
        self.engine.with_timeline(|timeline| {
            let samples_per_beat = track_time_params(timeline);
            let idx = timeline.tracks.len();
            timeline.add_track(TrackKind::Midi, &full_name);

            let Some(track) = timeline.tracks.last_mut() else { return Some(idx) };
            track.instrument_id = Some(inst_id);

            let length_samples = (samples_per_beat * 16.0) as u64;
            let mut clip = MidiClip::new(ClipId(clip_id), length_samples);
            clip.name = clip_label.clone();
            track.add_midi_clip(clip);

            Some(idx)
        }).flatten()
    }

    pub(super) fn load_native_drum(&mut self, inst_id_str: &str) {
        if inst_id_str != "drum808" {
            return;
        }

        let sample_rate = self.engine.sample_rate() as f32;
        let drum808 = Drum808::new(sample_rate);

        let inst_id = self.next_instrument_id;
        self.next_instrument_id += 1;

        self.engine.add_instrument(inst_id, Instrument::Drum808(drum808));

        let clip_id = self.next_clip_id;
        self.next_clip_id += 1;

        let track_idx = self.ensure_midi_track(inst_id, "808 Drums", "Drum Pattern", clip_id);

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

    pub(super) fn load_vst3_effect(&mut self, info: &Vst3PluginInfo) {
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

    pub(super) fn load_instrument_to_track(&mut self, info: &Vst3PluginInfo) {
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

        let clip_id = self.next_clip_id;
        self.next_clip_id += 1;

        let track_name = format!("MIDI - {}", info.name);
        let track_idx = self.ensure_midi_track(inst_id, &track_name, "New MIDI Clip", clip_id);

        let Some(idx) = track_idx else { return };

        self.selected_track_idx = Some(idx);
        self.selected_clip = Some(SelectedClip::Midi {
            track_idx: idx,
            clip_id: ClipId(clip_id),
        });
        self.show_clip_editor = true;

        self.open_native_plugin_gui(
            inst_id,
            &info.info.path.to_string_lossy(),
            &info.info.unique_id,
            &info.name,
        );
        tracing::info!("Loaded instrument {} to track {}", info.name, idx);
    }

    /// Delete a clip from the timeline.
    pub(super) fn delete_selected_clip(&mut self, clip: SelectedClip) {
        match clip {
            SelectedClip::Midi { track_idx, clip_id } => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        track.midi_clips.retain(|c| c.id != clip_id);
                    }
                });
                tracing::info!("Deleted MIDI clip {:?} from track {}", clip_id, track_idx);
            }
            SelectedClip::Audio { track_idx, clip_id } => {
                self.engine.with_timeline(|timeline| {
                    if let Some(track) = timeline.tracks.get_mut(track_idx) {
                        track.clips.retain(|c| c.id != clip_id);
                    }
                });
                tracing::info!("Deleted audio clip {:?} from track {}", clip_id, track_idx);
            }
        }
        self.show_clip_editor = false;
    }
}

/// Compute samples-per-beat from timeline transport.
fn track_time_params(timeline: &hallucinator_core::Timeline) -> f64 {
    let sample_rate = timeline.transport.sample_rate;
    let bpm = timeline.transport.bpm;
    sample_rate as f64 * 60.0 / bpm
}
