use hallucinator_core::{AudioClip, ClipId};
use hallucinator_services::wav_reader;

use super::config::{AppConfig, LibraryConfig, save_config};
use super::HallucinatorApp;

impl HallucinatorApp {
    pub(super) fn start_recording(&mut self) {
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

    pub(super) fn stop_recording(&mut self) {
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

    pub(super) fn load_audio_file(&mut self, path: &std::path::Path) {
        let (samples, channels, sample_rate) = match wav_reader::read_wav(path) {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Failed to open WAV file: {} - {}", path.display(), e);
                return;
            }
        };

        let mut clip = AudioClip::new(
            ClipId(self.next_clip_id),
            samples,
            sample_rate,
            channels,
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

    pub(super) fn save_library_config(&self) {
        let config = AppConfig {
            library: LibraryConfig {
                places: self.browser_panel.place_paths().iter().map(|p| p.display().to_string()).collect(),
            },
        };
        save_config(&config);
    }

    /// Read a WAV file to mono f32 samples.
    pub(super) fn read_wav_samples(path: &std::path::Path) -> Result<(Vec<f32>, u32), String> {
        wav_reader::read_wav_mono(path)
    }
}
