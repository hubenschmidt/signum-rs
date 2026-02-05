use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub(super) struct AppConfig {
    #[serde(default)]
    pub library: LibraryConfig,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub(super) struct LibraryConfig {
    #[serde(default)]
    pub places: Vec<String>,
}

pub(super) fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hallucinator")
        .join("config.toml")
}

pub(super) fn load_config() -> AppConfig {
    let path = config_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub(super) fn save_config(config: &AppConfig) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(s) = toml::to_string_pretty(config) else { return };
    let _ = std::fs::write(&path, s);
}
