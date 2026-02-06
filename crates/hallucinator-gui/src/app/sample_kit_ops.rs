use std::sync::Arc;

use hallucinator_services::{Instrument, SampleKit};

use crate::clipboard::ClipboardContent;
use super::HallucinatorApp;

impl HallucinatorApp {
    pub(super) fn copy_step_sample(&mut self, track_idx: usize, from_step: usize, from_layer: usize, to_step: usize, to_layer: usize) {
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();
        let Some(id) = inst_id else { return };

        let from_slot = from_step * 12 + from_layer;
        let to_slot = to_step * 12 + to_layer;

        let Ok(mut instruments) = self.engine_state.instruments.lock() else { return };
        let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&id) else { return };

        let (name, data) = {
            let slots = kit.slots();
            let Some(slot) = slots.get(from_slot).and_then(|s| s.as_ref()) else { return };
            (slot.name.clone(), Arc::clone(&slot.data))
        };

        kit.set_slot(to_slot, name.clone(), data);
        drop(instruments);

        self.keyboard_sequencer_panel.set_step_sample_name(to_step, to_layer, name);
    }

    pub(super) fn copy_drum_step_to_clipboard(&mut self, track_idx: usize, step: usize, layer: usize) {
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();
        let Some(id) = inst_id else { return };

        let slot_idx = step * 12 + layer;
        let Ok(instruments) = self.engine_state.instruments.lock() else { return };
        let Some(Instrument::SampleKit(kit)) = instruments.get(&id) else { return };
        let slots = kit.slots();
        let Some(slot) = slots.get(slot_idx).and_then(|s| s.as_ref()) else { return };

        self.clipboard.copy(ClipboardContent::SampleData {
            name: slot.name.clone(),
            data: Arc::clone(&slot.data),
        });
    }

    pub(super) fn paste_step_sample(
        &mut self,
        track_idx: usize,
        step: usize,
        layer: usize,
        name: String,
        data: Arc<Vec<f32>>,
    ) {
        let engine_sr = self.engine.sample_rate() as f32;
        let slot_idx = step * 12 + layer;

        let kit_id = self.get_or_create_sample_kit(track_idx, engine_sr);

        if let Ok(mut instruments) = self.engine_state.instruments.lock() {
            if let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&kit_id) {
                kit.set_slot(slot_idx, name.clone(), data);
            }
        }

        self.keyboard_sequencer_panel.set_step_sample_name(step, layer, name);
    }

    pub(super) fn load_step_sample(&mut self, track_idx: usize, step: usize, layer: usize, path: &std::path::Path) {
        let engine_sr = self.engine.sample_rate() as f32;

        let Ok((mono, _sample_rate)) = Self::read_wav_samples(path) else {
            tracing::error!("Failed to read WAV: {}", path.display());
            return;
        };

        let sample_name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("sample")
            .to_string();

        let data = Arc::new(mono);
        let slot_idx = step * 12 + layer;

        let kit_id = self.get_or_create_sample_kit(track_idx, engine_sr);

        if let Ok(mut instruments) = self.engine_state.instruments.lock() {
            if let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&kit_id) {
                kit.set_slot(slot_idx, sample_name.clone(), data);
            }
        }

        self.keyboard_sequencer_panel.set_step_sample_name(step, layer, sample_name);
        tracing::info!("Loaded sample to step {} layer {}: {}", step, layer, path.display());
    }

    pub(super) fn clear_step_sample(&mut self, track_idx: usize, step: usize, layer: usize) {
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();
        let Some(id) = inst_id else { return };
        let Ok(mut instruments) = self.engine_state.instruments.lock() else { return };
        let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&id) else { return };

        let slot_idx = step * 12 + layer;
        kit.clear_slot(slot_idx);
        tracing::debug!("Cleared sample at step {} layer {}", step, layer);
    }

    /// Load a sample for an entire row (slot index = row)
    pub(super) fn load_row_sample(&mut self, track_idx: usize, row: usize, path: &std::path::Path) {
        let engine_sr = self.engine.sample_rate() as f32;

        let Ok((mono, _sample_rate)) = Self::read_wav_samples(path) else {
            tracing::error!("Failed to read WAV: {}", path.display());
            return;
        };

        let sample_name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("sample")
            .to_string();

        let data = Arc::new(mono);
        let kit_id = self.get_or_create_sample_kit(track_idx, engine_sr);

        if let Ok(mut instruments) = self.engine_state.instruments.lock() {
            if let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&kit_id) {
                kit.set_slot(row, sample_name.clone(), data);
            }
        }

        self.keyboard_sequencer_panel.set_row_sample(row, sample_name);
        tracing::info!("Loaded row {} sample: {}", row, path.display());
    }

    /// Play a row's sample directly (for live triggering)
    pub(super) fn play_row_sample(&mut self, track_idx: usize, row: usize, velocity: u8) {
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();
        let Some(id) = inst_id else { return };
        let Ok(mut instruments) = self.engine_state.instruments.lock() else { return };
        let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&id) else { return };

        kit.trigger_slot(row, velocity);
    }

    /// Copy a row's sample to the clipboard
    pub(super) fn copy_row_sample_to_clipboard(&mut self, track_idx: usize, row: usize) {
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();
        let Some(id) = inst_id else { return };
        let Ok(instruments) = self.engine_state.instruments.lock() else { return };
        let Some(Instrument::SampleKit(kit)) = instruments.get(&id) else { return };
        let Some(slot) = kit.slots().get(row).and_then(|s| s.as_ref()) else { return };

        self.clipboard.copy(crate::clipboard::ClipboardContent::SampleData {
            name: slot.name.clone(),
            data: Arc::clone(&slot.data),
        });
        tracing::debug!("Copied row {} sample '{}' to clipboard", row, slot.name);
    }

    /// Paste sample from clipboard to a row
    pub(super) fn paste_row_sample_from_clipboard(&mut self, track_idx: usize, row: usize) {
        let Some(content) = self.clipboard.content() else { return };
        let (name, data) = match content {
            crate::clipboard::ClipboardContent::SampleData { name, data } => (name.clone(), Arc::clone(data)),
            crate::clipboard::ClipboardContent::FilePath(path) => {
                // Load from file path
                let Ok((mono, _)) = Self::read_wav_samples(&path) else { return };
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("sample").to_string();
                (name, Arc::new(mono))
            }
        };

        let engine_sr = self.engine.sample_rate() as f32;
        let kit_id = self.get_or_create_sample_kit(track_idx, engine_sr);

        if let Ok(mut instruments) = self.engine_state.instruments.lock() {
            if let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&kit_id) {
                kit.set_slot(row, name.clone(), data);
            }
        }

        self.keyboard_sequencer_panel.set_row_sample(row, name);
        tracing::debug!("Pasted sample to row {}", row);
    }

    /// Clear a row's sample
    pub(super) fn clear_row_sample(&mut self, track_idx: usize, row: usize) {
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();
        let Some(id) = inst_id else { return };
        let Ok(mut instruments) = self.engine_state.instruments.lock() else { return };
        let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&id) else { return };

        kit.clear_slot(row);
        self.keyboard_sequencer_panel.clear_row_sample(row);
        tracing::debug!("Cleared row {} sample", row);
    }

    /// Move sample from one row to another
    pub(super) fn move_row_sample(&mut self, track_idx: usize, from_row: usize, to_row: usize) {
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();
        let Some(id) = inst_id else { return };
        let Ok(mut instruments) = self.engine_state.instruments.lock() else { return };
        let Some(Instrument::SampleKit(kit)) = instruments.get_mut(&id) else { return };

        // Get source sample data (extract name and data before mutation)
        let Some(src_slot) = kit.slots().get(from_row).and_then(|s| s.as_ref()) else { return };
        let name = src_slot.name.clone();
        let data = Arc::clone(&src_slot.data);

        // Set destination and clear source
        kit.set_slot(to_row, name.clone(), data);
        kit.clear_slot(from_row);

        // Update panel state
        self.keyboard_sequencer_panel.set_row_sample(to_row, name);
        self.keyboard_sequencer_panel.clear_row_sample(from_row);
        tracing::debug!("Moved sample from row {} to row {}", from_row, to_row);
    }

    pub(super) fn get_or_create_sample_kit(&mut self, track_idx: usize, engine_sr: f32) -> u64 {
        let inst_id = self.engine.with_timeline(|t| {
            t.tracks.get(track_idx).and_then(|track| track.instrument_id)
        }).flatten();

        if let Some(id) = inst_id {
            let is_kit = self.engine_state.instruments.lock().ok()
                .and_then(|instruments| instruments.get(&id).map(|i| matches!(i, Instrument::SampleKit(_))))
                .unwrap_or(false);
            if is_kit { return id; }
        }

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
}
