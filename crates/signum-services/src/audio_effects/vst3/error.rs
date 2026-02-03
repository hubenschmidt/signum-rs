//! VST3 error types

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Vst3Error {
    #[error("Plugin not found: {0}")]
    NotFound(PathBuf),
    #[error("Failed to load plugin: {0}")]
    LoadError(String),
    #[error("Processing error: {0}")]
    ProcessError(String),
    #[error("No plugins found in scan paths")]
    NoPluginsFound,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
