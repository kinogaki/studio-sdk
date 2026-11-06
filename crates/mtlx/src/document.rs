//! [`Document`]: a MaterialX document, read from its XML ([`FromStr`],
//! upgrading MaterialX 1.36–1.38 to 1.39 as MaterialX's own reader does) and
//! written back ([`Display`](fmt::Display)) as MaterialX writes one. Nothing
//! read is lost: comments, blank lines, attributes no specification names
//! and `xi:include` elements are written back as they were.
use crate::upgrade;
use crate::xml::{self, Element};
use std::fmt;
use std::str::FromStr;

/// The version this crate reads and writes.
pub(crate) const VERSION: &str = "1.39";

/// A MaterialX document: its `<materialx>` element and all it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    root: Element,
}

impl FromStr for Document {
    type Err = String;

    /// `text` read; a document of MaterialX 1.36 to 1.38 is upgraded to
    /// 1.39 on the way. Refused: XML that doesn't read, a root other than
    /// `<materialx>`, or another version.
    fn from_str(text: &str) -> Result<Document, String> {
        let mut root = xml::parse(text)?;
        if root.category != "materialx" {
            return Err(format!("<{}> isn't a MaterialX document", root.category));
        }
        upgrade::upgrade(&mut root)?;
        Ok(Document { root })
    }
}

impl fmt::Display for Document {
    /// The document as MaterialX writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&xml::write(&self.root))
    }
}
