//! VST3 plugin scanner using rack crate

use std::path::Path;

use rack::{PluginInfo, PluginScanner, Scanner};
use tracing::info;

use super::error::Vst3Error;

/// Information about a discovered VST3 plugin
#[derive(Debug, Clone)]
pub struct Vst3PluginInfo {
    /// Plugin info from rack
    pub info: PluginInfo,
    /// Plugin name
    pub name: String,
}

/// Scanner for discovering VST3 plugins on the system
pub struct Vst3Scanner {
    scanner: Option<Scanner>,
    plugins: Vec<Vst3PluginInfo>,
}

impl Vst3Scanner {
    pub fn new() -> Result<Self, Vst3Error> {
        let scanner = Scanner::new().map_err(|e| Vst3Error::LoadError(format!("{:?}", e)))?;

        Ok(Self {
            scanner: Some(scanner),
            plugins: Vec::new(),
        })
    }

    /// Scan for VST3 plugins in default paths (`/usr/lib/vst3/`, `~/.vst3/`)
    pub fn scan(&mut self) -> Result<&[Vst3PluginInfo], Vst3Error> {
        self.plugins.clear();

        let scanner = self.scanner.as_ref().ok_or(Vst3Error::NoPluginsFound)?;
        let plugin_infos = scanner
            .scan()
            .map_err(|e| Vst3Error::LoadError(format!("{:?}", e)))?;

        self.add_plugins(plugin_infos);

        if self.plugins.is_empty() {
            return Err(Vst3Error::NoPluginsFound);
        }

        info!(count = self.plugins.len(), "Plugin scan complete");
        Ok(&self.plugins)
    }

    /// Scan a specific directory for VST3 plugins
    pub fn scan_path(&mut self, path: &Path) -> Result<&[Vst3PluginInfo], Vst3Error> {
        self.plugins.clear();

        let scanner = self.scanner.as_ref().ok_or(Vst3Error::NoPluginsFound)?;
        let plugin_infos = scanner
            .scan_path(path)
            .map_err(|e| Vst3Error::LoadError(format!("{:?}", e)))?;

        self.add_plugins(plugin_infos);

        if self.plugins.is_empty() {
            return Err(Vst3Error::NoPluginsFound);
        }

        info!(path = %path.display(), count = self.plugins.len(), "Plugin scan complete");
        Ok(&self.plugins)
    }

    fn add_plugins(&mut self, plugin_infos: Vec<PluginInfo>) {
        for plugin_info in plugin_infos {
            let name = plugin_info.name.clone();
            info!(name = %name, "Found plugin");
            self.plugins.push(Vst3PluginInfo {
                info: plugin_info,
                name,
            });
        }
    }

    pub fn plugins(&self) -> &[Vst3PluginInfo] {
        &self.plugins
    }

    pub fn find_by_name(&self, name: &str) -> Option<&Vst3PluginInfo> {
        self.plugins.iter().find(|p| p.name == name)
    }

    /// Get the internal rack Scanner for loading plugins
    pub fn scanner(&self) -> Option<&Scanner> {
        self.scanner.as_ref()
    }
}

impl Default for Vst3Scanner {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            scanner: None,
            plugins: Vec::new(),
        })
    }
}
