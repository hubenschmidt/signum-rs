use hallucinator_services::audio_effects::EffectParam;

use super::HallucinatorApp;

impl HallucinatorApp {
    /// Open native plugin GUI window directly (no egui parameter window).
    pub(super) fn open_native_plugin_gui(&mut self, plugin_id: u64, plugin_path: &str, plugin_uid: &str, title: &str) {
        if plugin_path.is_empty() || plugin_uid.is_empty() {
            tracing::warn!("Cannot open native GUI: missing plugin path or UID");
            return;
        }

        tracing::info!("Opening native GUI for plugin_id={} title={}", plugin_id, title);

        if let Err(e) = self.gui_manager.create_window(plugin_id, plugin_path, plugin_uid, title, 800, 600) {
            tracing::error!("Failed to create native plugin window: {}", e);
            return;
        }
        if let Err(e) = self.gui_manager.show_window(plugin_id) {
            tracing::warn!("Failed to show native plugin window: {}", e);
            return;
        }
        tracing::info!("Opened native GUI for plugin {}", title);
    }
}

/// Render parameter sliders for a list of effect params.
/// Returns a vec of (param_name, new_value) for any changed params.
pub(super) fn render_param_sliders(ui: &mut egui::Ui, params: &[EffectParam]) -> Vec<(String, f32)> {
    let mut changes = Vec::new();

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
            changes.push((param.name.clone(), value));
        }
        ui.add_space(4.0);
    }

    changes
}
