//! Browser panel - left sidebar with plugin/sound categories and Places

use std::path::PathBuf;

use egui::{CollapsingHeader, ScrollArea, Ui};
use hallucinator_services::Vst3PluginInfo;

// ── Native instrument info ──────────────────────────────────────────

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

// ── Sample library Places ───────────────────────────────────────────

/// A file or folder entry in the sample library tree.
struct LibEntry {
    name: String,
    path: PathBuf,
    children: Vec<LibEntry>,
    is_dir: bool,
}

/// A user-added folder ("Place") in the browser.
struct Place {
    root: PathBuf,
    name: String,
    entries: Vec<LibEntry>,
}

impl Place {
    fn scan(root: PathBuf) -> Option<Self> {
        if !root.is_dir() {
            return None;
        }
        let name = root.file_name()?.to_str()?.to_string();
        let entries = scan_dir(&root);
        Some(Self { root, name, entries })
    }
}

fn scan_dir(dir: &std::path::Path) -> Vec<LibEntry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in read.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(String::from) else {
            continue;
        };

        if path.is_dir() {
            let children = scan_dir(&path);
            // Only include dirs that contain at least one WAV (directly or nested)
            if has_any_wav(&children) {
                dirs.push(LibEntry { name, path, children, is_dir: true });
            }
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "wav" {
            files.push(LibEntry { name, path, children: Vec::new(), is_dir: false });
        }
    }

    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    dirs.extend(files);
    dirs
}

fn has_any_wav(entries: &[LibEntry]) -> bool {
    entries.iter().any(|e| !e.is_dir || has_any_wav(&e.children))
}

fn has_matching_descendant(entry: &LibEntry, filter: &str) -> bool {
    entry.children.iter().any(|child| {
        if child.is_dir {
            return has_matching_descendant(child, filter);
        }
        child.name.to_lowercase().contains(filter)
    })
}

fn render_tree(
    ui: &mut Ui,
    entries: &[LibEntry],
    filter: &str,
    selected_id: &mut Option<egui::Id>,
    action: &mut BrowserAction,
) {
    for entry in entries {
        if entry.is_dir {
            if !filter.is_empty() && !has_matching_descendant(entry, filter) {
                continue;
            }
            CollapsingHeader::new(&entry.name)
                .id_salt(&entry.path)
                .default_open(false)
                .show(ui, |ui| {
                    render_tree(ui, &entry.children, filter, selected_id, action);
                });
            continue;
        }
        if !filter.is_empty() && !entry.name.to_lowercase().contains(filter) {
            continue;
        }
        let item_id = egui::Id::new(&entry.path);

        ui.horizontal(|ui| {
            // Preview button
            if ui.small_button("▶").on_hover_text("Preview").clicked() {
                *action = BrowserAction::PreviewSample(entry.path.clone());
            }

            // File name (clickable, draggable)
            let resp = browser_item(ui, &entry.name, *selected_id == Some(item_id), true);
            if resp.clicked() {
                *selected_id = Some(item_id);
                *action = BrowserAction::SelectFile(entry.path.clone());
            }
            if resp.dragged() {
                egui::DragAndDrop::set_payload(ui.ctx(), entry.path.clone());
            }
            resp.context_menu(|ui| {
                if ui.button("Copy").clicked() {
                    *selected_id = Some(item_id);
                    *action = BrowserAction::SelectFile(entry.path.clone());
                    ui.close_menu();
                }
            });
        });
    }
}

/// Render a single browser row with selection highlight, hover, and optional drag support.
fn browser_item(ui: &mut Ui, label: &str, is_selected: bool, draggable: bool) -> egui::Response {
    let sense = if draggable {
        egui::Sense::click_and_drag()
    } else {
        egui::Sense::click()
    };
    let padding = ui.spacing().button_padding;
    let wrap_width = ui.available_width() - padding.x * 2.0;
    let text = egui::WidgetText::from(label);
    let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), wrap_width, egui::TextStyle::Button);
    let desired = egui::vec2(ui.available_width(), galley.size().y + padding.y * 2.0);
    let (rect, resp) = ui.allocate_exact_size(desired, sense);

    if is_selected {
        ui.painter().rect_filled(rect, 2.0, ui.visuals().selection.bg_fill);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 2.0, ui.visuals().widgets.hovered.bg_fill);
    }

    let text_color = if is_selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().text_color()
    };
    ui.painter().galley(rect.min + padding, galley, text_color);
    resp
}

// ── BrowserAction ───────────────────────────────────────────────────

/// Action returned from browser panel
#[derive(Clone)]
pub enum BrowserAction {
    None,
    LoadEffect(Vst3PluginInfo),
    LoadInstrument(Vst3PluginInfo),
    LoadNativeInstrument(NativeInstrumentInfo),
    SelectFile(PathBuf),
    PreviewSample(PathBuf),
    AddPlace(PathBuf),
    RemovePlace(usize),
}

// ── BrowserPanel ────────────────────────────────────────────────────

/// Browser panel state
pub struct BrowserPanel {
    filter_text: String,
    places: Vec<Place>,
    selected_id: Option<egui::Id>,
}

impl BrowserPanel {
    pub fn new() -> Self {
        Self {
            filter_text: String::new(),
            places: Vec::new(),
            selected_id: None,
        }
    }

    /// Initialize places from a list of folder paths.
    pub fn set_places(&mut self, paths: Vec<PathBuf>) {
        self.places = paths.into_iter().filter_map(Place::scan).collect();
    }

    /// Add a single folder as a new place. Returns false if invalid or duplicate.
    pub fn add_place(&mut self, path: PathBuf) -> bool {
        if self.places.iter().any(|p| p.root == path) {
            return false;
        }
        let Some(place) = Place::scan(path) else {
            return false;
        };
        self.places.push(place);
        true
    }

    /// Remove a place by index.
    pub fn remove_place(&mut self, index: usize) {
        if index < self.places.len() {
            self.places.remove(index);
        }
    }

    /// Return the root paths of all current places (for config persistence).
    pub fn place_paths(&self) -> Vec<PathBuf> {
        self.places.iter().map(|p| p.root.clone()).collect()
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
            // ── Categories ──────────────────────────────────────
            ui.strong("Categories");
            ui.add_space(2.0);

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

            // Drums section - native drum instruments
            CollapsingHeader::new("🥁 Drums")
                .default_open(true)
                .show(ui, |ui| {
                    for inst in NATIVE_DRUMS {
                        let name_lower = inst.name.to_lowercase();
                        if !filter_lower.is_empty() && !name_lower.contains(&filter_lower) {
                            continue;
                        }
                        let item_id = egui::Id::new(("native", inst.id));
                        let resp = browser_item(ui, inst.name, self.selected_id == Some(item_id), false);
                        if resp.clicked() {
                            self.selected_id = Some(item_id);
                        }
                        if resp.double_clicked() {
                            action = BrowserAction::LoadNativeInstrument(inst.clone());
                        }
                    }
                });

            // ── Places ──────────────────────────────────────────
            ui.add_space(8.0);
            ui.strong("Places");
            ui.separator();

            let mut remove_idx: Option<usize> = None;

            for (idx, place) in self.places.iter().enumerate() {
                let resp = CollapsingHeader::new(format!("📁 {}", place.name))
                    .id_salt(&place.root)
                    .default_open(false)
                    .show(ui, |ui| {
                        render_tree(ui, &place.entries, &filter_lower, &mut self.selected_id, &mut action);
                    });

                resp.header_response.context_menu(|ui| {
                    if ui.button("Remove").clicked() {
                        remove_idx = Some(idx);
                        ui.close_menu();
                    }
                });
            }

            if let Some(idx) = remove_idx {
                action = BrowserAction::RemovePlace(idx);
            }

            if ui.button("＋ Add Folder...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    action = BrowserAction::AddPlace(path);
                }
            }
        });

        action
    }

    fn show_plugin_list(
        &mut self,
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

            let item_id = egui::Id::new(("plugin", &plugin.name));
            let resp = browser_item(ui, &plugin.name, self.selected_id == Some(item_id), false);
            if resp.clicked() {
                self.selected_id = Some(item_id);
            }
            if resp.double_clicked() {
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
