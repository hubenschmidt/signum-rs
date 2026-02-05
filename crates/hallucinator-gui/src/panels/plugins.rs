//! Plugin browser panel for VST3 plugins

use std::path::PathBuf;

use egui::{Color32, ScrollArea, Ui};
use hallucinator_services::{Vst3PluginInfo, Vst3Scanner};
use tracing::info;

/// Action returned from plugin panel
pub enum PluginAction {
    None,
    LoadPlugin(Vst3PluginInfo),
    CreateMidiTrack(Vst3PluginInfo),
    AddAudioTrack,
    AddMidiTrack,
}

/// Plugin browser panel
pub struct PluginBrowserPanel {
    scanner: Option<Vst3Scanner>,
    plugins: Vec<Vst3PluginInfo>,
    scan_error: Option<String>,
    custom_path: String,
    use_custom_path: bool,
}

impl PluginBrowserPanel {
    pub fn new() -> Self {
        let default_path = dirs::home_dir()
            .map(|h| h.join(".vst3"))
            .unwrap_or_else(|| PathBuf::from("~/.vst3"));

        let mut panel = Self {
            scanner: None,
            plugins: Vec::new(),
            scan_error: None,
            custom_path: default_path.display().to_string(),
            use_custom_path: false,
        };

        // Auto-scan on init
        panel.scan();
        panel
    }

    /// Initialize scanner and scan for plugins
    pub fn scan(&mut self) {
        match Vst3Scanner::new() {
            Ok(mut scanner) => {
                let result = if self.use_custom_path {
                    scanner.scan_path(&PathBuf::from(&self.custom_path))
                } else {
                    scanner.scan()
                };

                match result {
                    Ok(plugins) => {
                        self.plugins = plugins.to_vec();
                        self.scan_error = None;
                    }
                    Err(e) => {
                        self.scan_error = Some(format!("{}", e));
                        self.plugins.clear();
                    }
                }
                self.scanner = Some(scanner);
            }
            Err(e) => {
                self.scan_error = Some(format!("{}", e));
            }
        }
    }

    /// Get the scanner for loading plugins
    pub fn scanner(&self) -> Option<&Vst3Scanner> {
        self.scanner.as_ref()
    }

    /// Render as a menu bar
    pub fn menu_ui(&mut self, ui: &mut Ui, snap_to_grid: &mut bool) -> PluginAction {
        let mut action = PluginAction::None;

        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            });

            ui.menu_button("Track", |ui| {
                if ui.button("Add Audio Track").clicked() {
                    action = PluginAction::AddAudioTrack;
                    ui.close_menu();
                }
                if ui.button("Add MIDI Track").clicked() {
                    action = PluginAction::AddMidiTrack;
                    ui.close_menu();
                }
            });

            ui.menu_button("Grid", |ui| {
                ui.checkbox(snap_to_grid, "Snap to Grid");
            });

            ui.menu_button("Plugins", |ui| {
                // Scan path settings
                ui.checkbox(&mut self.use_custom_path, "Use custom path");

                ui.horizontal(|ui| {
                    ui.label("Path:");
                    ui.add_enabled(
                        self.use_custom_path,
                        egui::TextEdit::singleline(&mut self.custom_path).desired_width(250.0),
                    );
                });

                ui.separator();

                if ui.button("Scan for Plugins").clicked() {
                    self.scan();
                }

                ui.separator();

                if let Some(err) = &self.scan_error {
                    ui.colored_label(Color32::RED, err);
                    ui.separator();
                }

                if self.plugins.is_empty() {
                    ui.label("No plugins found");
                    ui.label("Default paths: /usr/lib/vst3/, ~/.vst3/");
                } else {
                    ui.label(format!("{} plugins available:", self.plugins.len()));
                    ui.separator();

                    ui.label("Effects (add to master chain):");
                    ScrollArea::vertical().id_salt("effects_scroll").max_height(150.0).show(ui, |ui| {
                        for plugin in &self.plugins {
                            if ui.button(format!("🎛 {}", plugin.name)).clicked() {
                                info!("Plugin clicked as effect: {}", plugin.name);
                                action = PluginAction::LoadPlugin(plugin.clone());
                                ui.close_menu();
                            }
                        }
                    });

                    ui.separator();
                    ui.label("Instruments (create MIDI track):");
                    ScrollArea::vertical().id_salt("instruments_scroll").max_height(150.0).show(ui, |ui| {
                        for plugin in &self.plugins {
                            if ui.button(format!("🎹 {}", plugin.name)).clicked() {
                                info!("Plugin clicked as instrument: {}", plugin.name);
                                action = PluginAction::CreateMidiTrack(plugin.clone());
                                ui.close_menu();
                            }
                        }
                    });
                }
            });

            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    ui.close_menu();
                }
            });
        });

        action
    }
}

impl Default for PluginBrowserPanel {
    fn default() -> Self {
        Self::new()
    }
}
