//! Browser panel - left sidebar with plugin/sound categories

use egui::{CollapsingHeader, ScrollArea, Ui};
use signum_services::Vst3PluginInfo;

/// Info about a native (built-in) instrument
#[derive(Clone)]
pub struct NativeInstrumentInfo {
    pub id: &'static str,
    pub name: &'static str,
}

/// Available native instruments
pub const NATIVE_DRUMS: &[NativeInstrumentInfo] = &[
    NativeInstrumentInfo { id: "drum808", name: "808 Drums" },
];

/// Action returned from browser panel
#[derive(Clone)]
pub enum BrowserAction {
    None,
    LoadEffect(Vst3PluginInfo),
    LoadInstrument(Vst3PluginInfo),
    LoadNativeInstrument(NativeInstrumentInfo),
}

/// Browser panel state
pub struct BrowserPanel {
    filter_text: String,
}

impl BrowserPanel {
    pub fn new() -> Self {
        Self {
            filter_text: String::new(),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, plugins: &[Vst3PluginInfo]) -> BrowserAction {
        let mut action = BrowserAction::None;

        ui.heading("Browser");
        ui.separator();

        // Search filter
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.text_edit_singleline(&mut self.filter_text);
        });

        ui.separator();

        let filter_lower = self.filter_text.to_lowercase();

        ScrollArea::vertical().show(ui, |ui| {
            // Instruments section
            CollapsingHeader::new("🎹 Instruments")
                .default_open(true)
                .show(ui, |ui| {
                    action = self.show_plugin_list(ui, plugins, true, &action);
                });

            // Audio Effects section
            CollapsingHeader::new("🎛 Audio Effects")
                .default_open(true)
                .show(ui, |ui| {
                    action = self.show_plugin_list(ui, plugins, false, &action);
                });

            // Sounds section (placeholder for future sample browser)
            CollapsingHeader::new("🔊 Sounds")
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("Drag WAV files to import");
                });

            // Drums section - native drum instruments
            CollapsingHeader::new("🥁 Drums")
                .default_open(true)
                .show(ui, |ui| {
                    for inst in NATIVE_DRUMS {
                        let name_lower = inst.name.to_lowercase();
                        if !filter_lower.is_empty() && !name_lower.contains(&filter_lower) {
                            continue;
                        }
                        if ui.selectable_label(false, inst.name).double_clicked() {
                            action = BrowserAction::LoadNativeInstrument(inst.clone());
                        }
                    }
                });
        });

        action
    }

    fn show_plugin_list(
        &self,
        ui: &mut Ui,
        plugins: &[Vst3PluginInfo],
        is_instrument: bool,
        current_action: &BrowserAction,
    ) -> BrowserAction {
        let mut action = current_action.clone();

        let filter_lower = self.filter_text.to_lowercase();

        for plugin in plugins {
            let name_lower = plugin.name.to_lowercase();
            if !filter_lower.is_empty() && !name_lower.contains(&filter_lower) {
                continue;
            }

            if ui.selectable_label(false, &plugin.name).double_clicked() {
                action = if is_instrument {
                    BrowserAction::LoadInstrument(plugin.clone())
                } else {
                    BrowserAction::LoadEffect(plugin.clone())
                };
            }
        }

        if plugins.is_empty() {
            ui.label("No plugins scanned");
            ui.label("Use Plugins menu to scan");
        }

        action
    }
}

impl Default for BrowserPanel {
    fn default() -> Self {
        Self::new()
    }
}
