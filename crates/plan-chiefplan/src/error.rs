//! Error type for the template scanner.

use std::fmt;
use std::io;

/// Everything that can go wrong while reading a Chief template.
#[derive(Debug)]
pub enum Error {
    /// An operating-system level I/O failure.
    Io(io::Error),
    /// The file does not follow the layout this reader knows (bad magic,
    /// truncated header, ...).
    Format(String),
    /// Writing the JSON inventory failed.
    Json(serde_json::Error),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Format(m) => write!(f, "not a Chief template: {m}"),
            Error::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}
