//! Error type shared by every module of the crate.

use std::fmt;
use std::io;

/// Everything that can go wrong while reading Chief library files.
#[derive(Debug)]
pub enum Error {
    /// An operating-system level I/O failure.
    Io(io::Error),
    /// A JSON document (registry, cache, or `AssociatedData` payload) failed to parse.
    Json(serde_json::Error),
    /// The file does not follow the format it claims (truncated, bad pointer, ...).
    Corrupt(String),
    /// The file is valid but uses a feature this reader deliberately does not
    /// implement (WAL mode, `WITHOUT ROWID`, non-UTF-8 text, zip encryption, ...).
    Unsupported(String),
    /// A named table, column, entry, or object does not exist.
    NotFound(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn corrupt(msg: impl Into<String>) -> Self {
        Error::Corrupt(msg.into())
    }

    pub(crate) fn unsupported(msg: impl Into<String>) -> Self {
        Error::Unsupported(msg.into())
    }

    pub(crate) fn not_found(msg: impl Into<String>) -> Self {
        Error::NotFound(msg.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Json(e) => write!(f, "JSON error: {e}"),
            Error::Corrupt(m) => write!(f, "corrupt data: {m}"),
            Error::Unsupported(m) => write!(f, "unsupported: {m}"),
            Error::NotFound(m) => write!(f, "not found: {m}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Json(e) => Some(e),
            _ => None,
        }
    }
}

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
