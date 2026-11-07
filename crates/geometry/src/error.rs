//! [`Error`]: why a call failed, as one [`Diagnostic`] for the user and
//! the [`ErrorKind`] a caller branches on.
use crate::diagnostic::{Diagnostic, TextSpan};
use std::fmt;

/// Why a call failed: what the user reads ([`diagnostic`](Self::diagnostic),
/// an error saying what failed and where) and the kind of failure, for
/// the caller that acts on it. Every fallible call of the SDK returns it:
/// a value's constructor, a kernel's operation, a node's cook, a reader, a
/// compiler, a package's admission. Nothing the failed call would have
/// produced is kept.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Error {
    /// What kind of failure it is.
    pub kind: ErrorKind,
    /// What failed, for the user: its message, and the place in a text or
    /// the element it concerns.
    pub diagnostic: Diagnostic,
}

/// What kind of failure an [`Error`] is: what its caller does differs by
/// it, while the user reads the diagnostic alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The input can't be used: a value breaking a rule or passing a limit
    /// (ids past the largest), a malformed file, a stored value of the
    /// wrong shape, data of another type, a request naming what isn't there
    /// (a view an engine doesn't have), a file that isn't there or that the
    /// system couldn't read or write.
    Invalid,
    /// What the work needs isn't installed (a light of a type no installed
    /// engine declares). Reading a project, the host keeps the object as it
    /// was read and reports the message.
    Missing,
    /// A package was refused at admission (a duplicate id, a data type no
    /// package registered); the message says which package and why.
    Refused,
    /// The work saw its cancellation raised and stopped, or whoever was to
    /// answer let the request go; the caller drops it.
    Cancelled,
}

impl Error {
    /// A `kind` failure saying `message`.
    pub fn new(kind: ErrorKind, message: impl fmt::Display) -> Self {
        Self {
            kind,
            diagnostic: Diagnostic::error(message.to_string()),
        }
    }

    /// An [`ErrorKind::Invalid`] failure saying `message`.
    pub fn invalid(message: impl fmt::Display) -> Self {
        Self::new(ErrorKind::Invalid, message)
    }

    /// An [`ErrorKind::Missing`] failure saying `message`: what isn't
    /// installed, plainly ("light type acme.lights.rim isn't installed").
    pub fn missing(message: impl fmt::Display) -> Self {
        Self::new(ErrorKind::Missing, message)
    }

    /// An [`ErrorKind::Refused`] failure saying `message`.
    pub fn refused(message: impl fmt::Display) -> Self {
        Self::new(ErrorKind::Refused, message)
    }

    /// An [`ErrorKind::Cancelled`] failure.
    pub fn cancelled() -> Self {
        Self::new(ErrorKind::Cancelled, "cancelled")
    }

    /// Whether it is a cancellation, which a caller drops rather than
    /// shows.
    pub fn is_cancelled(&self) -> bool {
        self.kind == ErrorKind::Cancelled
    }

    /// Concerning element `element` (the lowest failing one, in the
    /// numbering of what was checked).
    pub fn element(mut self, element: usize) -> Self {
        self.diagnostic.element = Some(element);
        self
    }

    /// Pointing at `span` of a text.
    pub fn at(mut self, span: TextSpan) -> Self {
        self.diagnostic.span = Some(span);
        self
    }

    /// What failed, for the user, without the place it concerns.
    pub fn message(&self) -> &str {
        &self.diagnostic.message
    }
}

/// An [`ErrorKind::Invalid`] failure reporting `diagnostic`.
impl From<Diagnostic> for Error {
    fn from(diagnostic: Diagnostic) -> Self {
        Self {
            kind: ErrorKind::Invalid,
            diagnostic,
        }
    }
}

/// An [`ErrorKind::Invalid`] failure with the system's message.
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::invalid(error)
    }
}

/// The diagnostic: "line 7, column 12: message", or the message.
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.diagnostic.fmt(f)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_is_its_diagnostic_and_its_kind() {
        let e = Error::invalid("polygon repeats a point").element(12);
        assert_eq!(e.to_string(), "polygon repeats a point");
        assert_eq!(
            (e.kind, e.message()),
            (ErrorKind::Invalid, "polygon repeats a point")
        );
        assert!(Error::cancelled().is_cancelled());
        let read = Error::from(std::io::Error::other("no such file"));
        assert_eq!(
            (read.kind, read.message()),
            (ErrorKind::Invalid, "no such file")
        );
    }
}
