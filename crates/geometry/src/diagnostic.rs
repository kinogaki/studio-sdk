//! [`Diagnostic`]: what the user reads about a problem a cook, a reader or
//! a check found: shown on its node, listed in the Console and underlined
//! in a code field.
use std::fmt;

/// How serious a diagnostic is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The work failed.
    #[default]
    Error,
    /// The work went on, but something was skipped or may surprise.
    Warning,
    /// A note.
    Info,
}

/// Where in a text (a code parameter) a diagnostic points: a line and a
/// column, both from 1 (columns in characters), and how many characters it
/// covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextSpan {
    /// The line, from 1.
    pub line: usize,
    /// The first character's column, from 1.
    pub column: usize,
    /// The characters covered; 0 points between two characters.
    pub length: usize,
}

impl TextSpan {
    /// `length` characters from `column` of `line`.
    pub fn new(line: usize, column: usize, length: usize) -> Self {
        Self {
            line,
            column,
            length,
        }
    }
}

/// A problem and what it concerns: a place in a text, an element of the
/// value (a point, a face) or neither.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    /// How serious it is.
    pub severity: Severity,
    /// What is wrong, in a sentence, for the user.
    pub message: String,
    /// Where in a code parameter's text it points.
    pub span: Option<TextSpan>,
    /// The element it concerns (the lowest failing one), in the numbering
    /// of what was checked: a value's point or primitive in its data
    /// type's own numbering, a compiled network's node by its
    /// host id (`NodeRef.0`).
    pub element: Option<usize>,
}

impl Diagnostic {
    /// A `severity` diagnostic saying `message`, pointing nowhere.
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            span: None,
            element: None,
        }
    }

    /// An error saying `message`.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Severity::Error, message)
    }

    /// A warning saying `message`.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, message)
    }

    /// A note saying `message`.
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(Severity::Info, message)
    }

    /// Pointing at `span`.
    pub fn at(mut self, span: TextSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Concerning element `element`.
    pub fn element(mut self, element: usize) -> Self {
        self.element = Some(element);
        self
    }
}

/// "line 7, column 12: message" or the message. An element is shown, not
/// written: a message names what it concerns in its own words ("polygon
/// 12 repeats a point").
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(span) = self.span {
            write!(f, "line {}, column {}: ", span.line, span.column)?;
        }
        f.write_str(&self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_say_where() {
        let d = Diagnostic::error("unknown name `y`").at(TextSpan::new(7, 12, 1));
        assert_eq!(d.to_string(), "line 7, column 12: unknown name `y`");
        assert_eq!(Diagnostic::warning("NaN").element(4).to_string(), "NaN");
        assert_eq!(Diagnostic::info("ok").severity, Severity::Info);
    }
}
