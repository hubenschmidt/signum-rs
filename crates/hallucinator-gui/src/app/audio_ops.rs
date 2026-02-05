use hallucinator_core::{AudioClip, ClipId};

use super::config::{AppConfig, LibraryConfig, save_config};
use super::HallucinatorApp;

pub(super) fn to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 { return samples.to_vec(); }
    samples.chunks(channels)
        .map(|ch| ch.iter().sum::<f32>() / channels as f32)
        .collect()
}

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

    pub(super) fn save_library_config(&self) {
        let config = AppConfig {
            library: LibraryConfig {
                places: self.browser_panel.place_paths().iter().map(|p| p.display().to_string()).collect(),
            },
        };
        save_config(&config);
    }

    /// Read a WAV file to mono f32 samples. Tries hound first, falls back to
    /// manual RIFF parsing for files with extended fmt chunks.
    pub(super) fn read_wav_samples(path: &std::path::Path) -> Result<(Vec<f32>, u32), String> {
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
}
