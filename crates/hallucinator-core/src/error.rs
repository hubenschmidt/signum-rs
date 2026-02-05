//! Error types for hallucinator

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HallucinatorError {
    #[error("Audio error: {0}")]
    Audio(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Track not found: {0}")]
    TrackNotFound(u64),
    #[error("Clip not found: {0}")]
    ClipNotFound(u64),
}

pub type Result<T> = std::result::Result<T, HallucinatorError>;
