//! [`Document`]: a MaterialX document, read from its XML ([`FromStr`],
//! upgrading MaterialX 1.36–1.38 to 1.39 as MaterialX's own reader does) and
//! written back ([`Display`](fmt::Display)) as MaterialX writes one. Nothing
//! read is lost: comments, blank lines, attributes no specification names
//! and `xi:include` elements are written back as they were.
use crate::upgrade;
use crate::xml::{self, Child, Element};
use std::fmt;
use std::str::FromStr;

/// The version this crate reads and writes.
pub(crate) const VERSION: &str = "1.39";

/// A MaterialX document: its `<materialx>` element and all it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub(crate) root: Element,
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

impl Document {
    /// Each `xi:include` at the document's top level replaced by the
    /// elements of the document it names (MaterialX's XInclude), upgraded as
    /// any document is, each holding the color space and file prefix the
    /// included document says: `load` gives the text of a path, an include's `href`
    /// joined to the folder of the document naming it (this one's: none). An
    /// included document's own includes are read in turn. What can't be
    /// read, or includes itself, is left out; what was left out, said.
    pub fn include(&mut self, load: &mut dyn FnMut(&str) -> Result<String, String>) -> Vec<String> {
        let mut said = Vec::new();
        let mut reading = Vec::new();
        include(&mut self.root, "", load, &mut reading, &mut said);
        said
    }
}

/// `root`'s includes replaced (see [`Document::include`]), `from` the
/// folder of the document holding it, `reading` the documents being read.
fn include(
    root: &mut Element,
    from: &str,
    load: &mut dyn FnMut(&str) -> Result<String, String>,
    reading: &mut Vec<String>,
    said: &mut Vec<String>,
) {
    let children = std::mem::take(&mut root.children);
    for child in children {
        let href = match &child {
            Child::Element(e) if e.category == "xi:include" => e.get("href").to_owned(),
            _ => {
                root.children.push(child);
                continue;
            }
        };
        let path = match href.starts_with('/') || from.is_empty() {
            true => href.clone(),
            false => format!("{from}/{href}"),
        };
        if reading.contains(&path) {
            said.push(format!("{path} includes itself; left out"));
            continue;
        }
        let read = load(&path).and_then(|text| text.parse::<Document>());
        let mut included = match read {
            Ok(document) => document.root,
            Err(why) => {
                said.push(format!("{path}: {why}; what it holds is left out"));
                continue;
            }
        };
        let folder = path
            .rsplit_once('/')
            .map_or("", |(folder, _)| folder)
            .to_owned();
        reading.push(path);
        include(&mut included, &folder, load, reading, said);
        reading.pop();
        // What the included document says of all it holds (its color space,
        // its file prefix) each of its elements now says.
        let inherited: Vec<(xml::Text, xml::Text)> = (included.attributes.iter())
            .filter(|(k, _)| matches!(&**k, "colorspace" | "fileprefix" | "geomprefix"))
            .cloned()
            .collect();
        for child in &mut included.children {
            if let Child::Element(e) = child {
                for (key, value) in &inherited {
                    if e.attr(key).is_none() {
                        e.set(key, value);
                    }
                }
            }
        }
        root.children.extend(included.children);
    }
}

impl fmt::Display for Document {
    /// The document as MaterialX writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&xml::write(&self.root))
    }
}
