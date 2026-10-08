//! Error type shared by every parser in this crate.

use std::fmt;

/// Why a configuration file could not be understood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Malformed XML (1-based line number and a short message).
    Xml { line: usize, msg: String },
    /// Well-formed text that does not follow the expected Chief format.
    Format(String),
    /// JSON (de)serialization failed.
    Json(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Xml { line, msg } => write!(f, "XML error at line {line}: {msg}"),
            ConfigError::Format(msg) => write!(f, "format error: {msg}"),
            ConfigError::Json(msg) => write!(f, "JSON error: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}
