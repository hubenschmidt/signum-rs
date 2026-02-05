//! VST3 effect wrapper implementing AudioEffect trait

use std::fmt;

use std::collections::HashMap;

use rack::{Plugin, PluginInstance, PluginScanner, Scanner};
use tracing::info;

use super::error::Vst3Error;
use super::scanner::Vst3PluginInfo;
use crate::audio_effects::{AudioEffect, EffectParam};

/// Wrapper around a VST3 plugin instance that implements AudioEffect
pub struct Vst3Effect {
    instance: Plugin,
    info: Vst3PluginInfo,
    bypassed: bool,
    sample_rate: f32,
    // Pre-allocated buffers for mono<->stereo conversion
    input_left: Vec<f32>,
    input_right: Vec<f32>,
    output_left: Vec<f32>,
    output_right: Vec<f32>,
    max_block_size: usize,
    // Parameter name -> index mapping
    param_map: HashMap<String, usize>,
    // Cached parameter info
    param_cache: Vec<EffectParam>,
}

// Safety: We ensure single-threaded access via Mutex<EffectChain>
unsafe impl Send for Vst3Effect {}

impl Vst3Effect {
    /// Create a new VST3 effect from plugin info using provided scanner
    pub fn new(
        scanner: &Scanner,
        info: &Vst3PluginInfo,
        sample_rate: f32,
    ) -> Result<Self, Vst3Error> {
        let mut instance = scanner
            .load(&info.info)
            .map_err(|e| Vst3Error::LoadError(format!("{:?}", e)))?;

        // Initialize with sample rate and max block size
        let max_block_size = 4096;
        instance
            .initialize(sample_rate as f64, max_block_size)
            .map_err(|e| Vst3Error::LoadError(format!("{:?}", e)))?;

        // Build parameter map
        let mut param_map = HashMap::new();
        let mut param_cache = Vec::new();
        let param_count = instance.parameter_count();

        for i in 0..param_count {
            if let Ok(pinfo) = instance.parameter_info(i) {
                param_map.insert(pinfo.name.clone(), i);
                let current_value = instance.get_parameter(i).unwrap_or(pinfo.default);
                param_cache.push(EffectParam::new(
                    &pinfo.name,
                    current_value,
                    pinfo.min,
                    pinfo.max,
                    &pinfo.unit,
                ));
            }
        }

        info!(name = %info.name, sample_rate, params = param_count, "VST3 plugin loaded");

        Ok(Self {
            instance,
            info: info.clone(),
            bypassed: false,
            sample_rate,
            input_left: vec![0.0; max_block_size],
            input_right: vec![0.0; max_block_size],
            output_left: vec![0.0; max_block_size],
            output_right: vec![0.0; max_block_size],
            max_block_size,
            param_map,
            param_cache,
        })
    }

    /// Get plugin info
    pub fn plugin_info(&self) -> &Vst3PluginInfo {
        &self.info
    }
}

impl fmt::Debug for Vst3Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vst3Effect")
            .field("name", &self.info.name)
            .field("bypassed", &self.bypassed)
            .finish()
    }
}

impl AudioEffect for Vst3Effect {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn process(&mut self, samples: &mut [f32]) {
        if self.bypassed {
            return;
        }

        let num_samples = samples.len().min(self.max_block_size);

        // Copy mono input to stereo buffers
        for i in 0..num_samples {
            self.input_left[i] = samples[i];
            self.input_right[i] = samples[i];
        }

        // Clear output buffers
        self.output_left[..num_samples].fill(0.0);
        self.output_right[..num_samples].fill(0.0);

        // Process through plugin (planar stereo format)
        let inputs: [&[f32]; 2] = [
            &self.input_left[..num_samples],
            &self.input_right[..num_samples],
        ];
        let mut outputs: [&mut [f32]; 2] = [
            &mut self.output_left[..num_samples],
            &mut self.output_right[..num_samples],
        ];

        if let Err(e) = self.instance.process(&inputs, &mut outputs, num_samples) {
            tracing::warn!("VST3 process error: {:?}", e);
            return;
        }

        // Mix stereo output back to mono
        for i in 0..num_samples {
            samples[i] = (self.output_left[i] + self.output_right[i]) * 0.5;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) {
        let Some(&index) = self.param_map.get(name) else {
            return;
        };

        if let Err(e) = self.instance.set_parameter(index, value) {
            tracing::warn!("Failed to set parameter {}: {:?}", name, e);
            return;
        }

        // Update cache
        if let Some(param) = self.param_cache.get_mut(index) {
            param.value = value;
        }
    }

    fn get_params(&self) -> Vec<EffectParam> {
        self.param_cache.clone()
    }

    fn set_bypass(&mut self, bypass: bool) {
        self.bypassed = bypass;
    }

    fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        if (sample_rate - self.sample_rate).abs() < 1.0 {
            return;
        }

        self.sample_rate = sample_rate;

        // Reinitialize with new sample rate
        if let Err(e) = self
            .instance
            .initialize(sample_rate as f64, self.max_block_size)
        {
            tracing::error!("Failed to reinitialize VST3: {:?}", e);
        }
    }
}
