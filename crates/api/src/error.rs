//! The one error type the API's fallible calls return.
use std::fmt;

/// Why a call into a package failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The work saw its [`CancelToken`](crate::CancelToken) raised and
    /// stopped; nothing it would have produced is kept.
    Cancelled,
    /// The input could not be used: a malformed or unsupported file, a
    /// stored value of the wrong shape, data of another type. The message
    /// says what and where, for the user.
    Invalid(String),
    /// Reading or writing a file failed (the operating system's message).
    Io(String),
}

impl Error {
    /// An [`Error::Invalid`] saying `message`.
    pub fn invalid(message: impl fmt::Display) -> Self {
        Self::Invalid(message.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("cancelled"),
            Self::Invalid(message) | Self::Io(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}
