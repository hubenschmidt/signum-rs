//! Manual RIFF/WAV parser that handles extended fmt chunks and various bit depths.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Read a WAV file to f32 samples. Returns (samples, channels, sample_rate).
pub fn read_wav(path: &Path) -> Result<(Vec<f32>, u16, u32), String> {
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
    // audio_format 1 = PCM, 3 = IEEE float, 65534 = WAVE_FORMAT_EXTENSIBLE
    if audio_format != 1 && audio_format != 3 && audio_format != 65534 {
        return Err(format!("unsupported format {audio_format}"));
    }

    let samples: Vec<f32> = match (audio_format, bits_per_sample) {
        (3, 32) => data_bytes.chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect(),
        (_, 16) => data_bytes.chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect(),
        (_, 24) => data_bytes.chunks_exact(3)
            .map(|b| {
                let val = (b[0] as i32) | ((b[1] as i32) << 8) | ((b[2] as i32) << 16);
                let signed = if val & 0x800000 != 0 { val | !0xFFFFFF } else { val };
                signed as f32 / 8388608.0
            })
            .collect(),
        (_, 32) => data_bytes.chunks_exact(4)
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2147483648.0)
            .collect(),
        _ => return Err(format!("unsupported bits {bits_per_sample}")),
    };

    Ok((samples, channels, sample_rate))
}

/// Read WAV and convert to mono. Returns (mono_samples, sample_rate).
pub fn read_wav_mono(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let (samples, channels, sample_rate) = read_wav(path)?;
    let mono = to_mono(&samples, channels as usize);
    Ok((mono, sample_rate))
}

/// Convert interleaved samples to mono by averaging channels.
pub fn to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 { return samples.to_vec(); }
    samples.chunks(channels)
        .map(|ch| ch.iter().sum::<f32>() / channels as f32)
        .collect()
}
