//! What a code field knows of its language: a [`CodeLanguage`] colors the
//! text by [`Token`]s, offers [`Completions`] at the cursor and explains
//! what the pointer rests on. The host's code editor knows no language.
use std::ops::Range;

/// What a run of code is, for its color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenClass {
    /// `if`, `for`, `def`, `param`.
    Keyword,
    /// `1`, `0.5`.
    Number,
    /// `"text"`.
    String,
    /// `# a note`.
    Comment,
    /// A variable or parameter.
    Name,
    /// An attribute (`pt.P`'s `P`).
    Attribute,
    /// A function called.
    Function,
    /// `+`, `=`, brackets.
    Operator,
    /// Text the language cannot read.
    Error,
}

/// A run of code of one class: a byte range of the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// The bytes it covers.
    pub range: Range<usize>,
    /// What it is.
    pub class: TokenClass,
}

/// One completion offered at the cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    /// The entry's text in the list.
    pub label: String,
    /// What accepting it writes in place of [`Completions::replace`].
    pub insert: String,
    /// Small text after the label (a signature or type).
    pub detail: String,
}

/// Completions at a cursor and the text they replace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Completions {
    /// The bytes an accepted entry replaces (the word typed so far).
    pub replace: Range<usize>,
    /// The entries, best first.
    pub items: Vec<Completion>,
}

/// A language as a code field reads it. The host calls it on the UI
/// thread while it draws the field, so each call must be quick. Every
/// method has a default that knows nothing, so plain text needs no
/// implementation of its own ([`PlainText`]).
pub trait CodeLanguage {
    /// `text` split into colored runs, in order and not overlapping; text
    /// in no run draws in the plain text color.
    fn tokens(&self, _text: &str) -> Vec<Token> {
        Vec::new()
    }

    /// What may be written at byte `cursor` of `text` (none: no popup).
    fn complete(&self, _text: &str, _cursor: usize) -> Completions {
        Completions::default()
    }

    /// What the code at byte `at` of `text` is (a function's signature),
    /// shown while the pointer rests there.
    fn hover(&self, _text: &str, _at: usize) -> Option<String> {
        None
    }
}

/// Plain text: no colors, completions or hover.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlainText;

impl CodeLanguage for PlainText {}
