//! [`Document`]: a MaterialX document, read from its XML ([`FromStr`],
//! upgrading MaterialX 1.36–1.38 to 1.39 as MaterialX's own reader does) and
//! written back ([`Display`](fmt::Display)) as MaterialX writes one. Nothing
//! read is lost: comments, blank lines, attributes no specification names
//! and `xi:include` elements are written back as they were.
use crate::upgrade;
use crate::xml::{self, Child, Element};
use std::collections::HashSet;
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

/// A document an `xi:include` names, found and read
/// ([`Document::include`]).
#[derive(Clone, Debug)]
pub struct Included {
    /// What tells it from any other file however it is named: its
    /// canonical path.
    pub identity: String,
    /// Its text.
    pub text: String,
}

impl Document {
    /// Each `xi:include` at the document's top level replaced by the
    /// elements of the document it names (MaterialX's XInclude), upgraded as
    /// any document is, each holding the color space and file prefix the
    /// included document says. A document has an identity, which tells one
    /// file from another however it is named (its canonical path): this
    /// one's is `identity`, none for one held in memory. `load(href, within)`
    /// finds and reads what `href` names, relative to the document of
    /// identity `within`, or none when it can't be read (the loader says
    /// why). An included document's own includes
    /// are read in turn, at most 256 deep. A document is included once, as
    /// MaterialX imports an element's name once: one included again is left
    /// out, and one including itself, or a document that doesn't read, is
    /// left out and said.
    pub fn include(
        &mut self,
        identity: Option<&str>,
        load: &mut dyn FnMut(&str, Option<&str>) -> Option<Included>,
    ) -> Vec<String> {
        let mut includes = Includes {
            load,
            reading: identity.into_iter().map(str::to_owned).collect(),
            read: identity.into_iter().map(str::to_owned).collect(),
            said: Vec::new(),
        };
        includes.replace(&mut self.root);
        includes.said
    }
}

/// The includes of one document being read.
struct Includes<'a> {
    load: &'a mut dyn FnMut(&str, Option<&str>) -> Option<Included>,
    /// The identities of the documents being read, each including the next.
    reading: Vec<String>,
    /// Every document read.
    read: HashSet<String>,
    said: Vec<String>,
}

impl Includes<'_> {
    /// `root`'s includes replaced (see [`Document::include`]).
    fn replace(&mut self, root: &mut Element) {
        let children = std::mem::take(&mut root.children);
        for child in children {
            let href = match &child {
                Child::Element(e) if e.category == "xi:include" => e.get("href").to_owned(),
                _ => {
                    root.children.push(child);
                    continue;
                }
            };
            if self.reading.len() >= xml::DEPTH {
                let depth = xml::DEPTH;
                self.said.push(format!(
                    "{href}: includes nested deeper than {depth}; left out"
                ));
                continue;
            }
            let within = self.reading.last().map(String::as_str);
            let Some(Included { identity, text }) = (self.load)(&href, within) else {
                continue;
            };
            if self.reading.contains(&identity) {
                self.said.push(format!("{href} includes itself; left out"));
                continue;
            }
            if !self.read.insert(identity.clone()) {
                continue;
            }
            let mut included = match text.parse::<Document>() {
                Ok(document) => document.root,
                Err(why) => {
                    self.said
                        .push(format!("{href}: {why}; what it holds is left out"));
                    continue;
                }
            };
            self.reading.push(identity);
            self.replace(&mut included);
            self.reading.pop();
            // What the included document says of all it holds (its color
            // space, its file prefix) each of its elements now says.
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
}

impl fmt::Display for Document {
    /// The document as MaterialX writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&xml::write(&self.root))
    }
}
