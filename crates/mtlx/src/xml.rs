//! The XML MaterialX documents are written in, read and written as
//! MaterialX's own reader and writer do (pugixml with MaterialX's comment and
//! newline options): elements and their attributes in order, comments kept
//! verbatim, and each blank line between elements kept as a [`Child::Newline`],
//! so a document written as MaterialX writes one reads back byte for byte.
//!
//! Attribute values are decoded (the five named entities and character
//! references) and their tabs and line breaks read as spaces, as pugixml's
//! default does. Declarations, processing instructions and a doctype are
//! skipped. Text between elements other than whitespace is refused: MaterialX
//! has none.
//!
//! Text read from a `'static` source (the embedded libraries) is borrowed, not
//! copied: the library reads in a few milliseconds.
//!
//! Elements nest at most [`DEPTH`] deep, so reading, writing and dropping a
//! document recurse within the 2 MiB stack a file is read on.
use std::borrow::Cow;

/// How deep elements nest: MaterialX's own documents nest a handful of
/// levels.
pub(crate) const DEPTH: usize = 256;

/// An element's text: borrowed from a `'static` source, else its own.
pub(crate) type Text = Cow<'static, str>;

/// An element: its category (the tag), its attributes in order and its
/// children.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Element {
    pub category: Text,
    pub attributes: Vec<(Text, Text)>,
    pub children: Vec<Child>,
}

/// What an element holds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Child {
    Element(Element),
    /// A comment's text, between `<!--` and `-->`.
    Comment(Text),
    /// A blank line between children.
    Newline,
}

impl Element {
    /// Attribute `name`'s value, when it is set.
    pub fn attr(&self, name: &str) -> Option<&str> {
        (self.attributes.iter()).find_map(|(k, v)| (k == name).then_some(&**v))
    }

    /// Attribute `name`'s value, empty when it isn't set.
    pub fn get(&self, name: &str) -> &str {
        self.attr(name).unwrap_or("")
    }

    /// Sets attribute `name` to `value`, in place when it is set, else last.
    pub fn set(&mut self, name: &str, value: &str) {
        match self.attributes.iter_mut().find(|(k, _)| k == name) {
            Some((_, v)) => *v = Cow::Owned(value.to_owned()),
            None => {
                (self.attributes).push((Cow::Owned(name.to_owned()), Cow::Owned(value.to_owned())))
            }
        }
    }

    /// Its child elements, in order.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Child::Element(e) => Some(e),
            _ => None,
        })
    }
}

/// `text` read: its root element, holding copies of its text.
pub(crate) fn parse(text: &str) -> Result<Element, String> {
    read(text, |s| Cow::Owned(s.to_owned()))
}

/// `text` read, its root element borrowing from it.
pub(crate) fn parse_static(text: &'static str) -> Result<Element, String> {
    read(text, Cow::Borrowed)
}

fn read<'a>(text: &'a str, keep: fn(&'a str) -> Text) -> Result<Element, String> {
    let mut reader = Reader {
        source: text,
        text: text.as_bytes(),
        at: 0,
        keep,
    };
    reader.prolog()?;
    let root = reader.element(1)?;
    reader.prolog()?;
    match reader.at < reader.text.len() {
        true => Err(reader.error("content after the root element")),
        false => Ok(root),
    }
}

/// `root` written as MaterialX writes a document, the declaration first.
pub(crate) fn write(root: &Element) -> String {
    let mut out = String::from("<?xml version=\"1.0\"?>\n");
    write_element(root, 0, &mut out);
    out
}

fn write_element(e: &Element, depth: usize, out: &mut String) {
    indent(depth, out);
    out.push('<');
    out.push_str(&e.category);
    // The name first, as MaterialX keeps it apart from the other attributes.
    let named = e.attributes.iter().filter(|(k, _)| k == "name");
    for (key, value) in named.chain(e.attributes.iter().filter(|(k, _)| k != "name")) {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        escape(value, out);
        out.push('"');
    }
    if e.children.is_empty() {
        out.push_str(" />\n");
        return;
    }
    out.push_str(">\n");
    for child in &e.children {
        match child {
            Child::Element(c) => write_element(c, depth + 1, out),
            Child::Comment(text) => {
                indent(depth + 1, out);
                out.push_str("<!--");
                // pugixml keeps a comment well formed: a `-` before another
                // or before the end is followed by a space.
                let bytes = text.as_bytes();
                for (k, ch) in text.char_indices() {
                    out.push(ch);
                    if ch == '-' && matches!(bytes.get(k + 1), Some(b'-') | None) {
                        out.push(' ');
                    }
                }
                out.push_str("-->\n");
            }
            Child::Newline => out.push('\n'),
        }
    }
    indent(depth, out);
    out.push_str("</");
    out.push_str(&e.category);
    out.push_str(">\n");
}

fn indent(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

/// An attribute value escaped as pugixml (MaterialX's build) escapes one:
/// `&` and `"` named, control characters by number, angle brackets kept.
fn escape(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            c if (c as u32) < 32 && c != '\t' => {
                let n = c as u32;
                out.push_str("&#");
                out.push(char::from(b'0' + (n / 10) as u8));
                out.push(char::from(b'0' + (n % 10) as u8));
                out.push(';');
            }
            c => out.push(c),
        }
    }
}

struct Reader<'a> {
    source: &'a str,
    text: &'a [u8],
    at: usize,
    keep: fn(&'a str) -> Text,
}

impl<'a> Reader<'a> {
    fn error(&self, what: &str) -> String {
        let line = 1 + self.text[..self.at.min(self.text.len())]
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        format!("line {line}: {what}")
    }

    fn rest(&self) -> &[u8] {
        &self.text[self.at.min(self.text.len())..]
    }

    fn starts(&self, s: &str) -> bool {
        self.rest().starts_with(s.as_bytes())
    }

    fn skip_space(&mut self) {
        while self.at < self.text.len() && self.text[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }

    /// Past `end`, from here: what came before it.
    fn until(&mut self, end: &str) -> Result<&'a str, String> {
        let found = (self.source[self.at..].find(end))
            .ok_or_else(|| self.error(&format!("no closing {end}")))?;
        let start = self.at;
        self.at += found + end.len();
        Ok(&self.source[start..start + found])
    }

    /// Whitespace, declarations, processing instructions, a doctype and
    /// comments outside the root.
    fn prolog(&mut self) -> Result<(), String> {
        loop {
            self.skip_space();
            if self.starts("<?") {
                self.until("?>")?;
            } else if self.starts("<!--") {
                self.at += 4;
                self.until("-->")?;
            } else if self.starts("<!DOCTYPE") {
                self.until(">")?;
            } else {
                return Ok(());
            }
        }
    }

    fn name(&mut self) -> Result<&'a str, String> {
        let start = self.at;
        while self.at < self.text.len() {
            let b = self.text[self.at];
            if b.is_ascii_whitespace() || matches!(b, b'=' | b'/' | b'>') {
                break;
            }
            self.at += 1;
        }
        // Names end at ASCII, so the slice stays on character boundaries.
        match self.at > start {
            true => Ok(&self.source[start..self.at]),
            false => Err(self.error("a name expected")),
        }
    }

    /// An element `depth` levels deep, its `<` next.
    fn element(&mut self, depth: usize) -> Result<Element, String> {
        if !self.starts("<") {
            return Err(self.error("an element expected"));
        }
        if depth > DEPTH {
            return Err(self.error(&format!("elements nested deeper than {DEPTH}")));
        }
        self.at += 1;
        let mut element = Element {
            category: (self.keep)(self.name()?),
            ..Element::default()
        };
        loop {
            self.skip_space();
            if self.starts("/>") {
                self.at += 2;
                return Ok(element);
            }
            if self.starts(">") {
                self.at += 1;
                break;
            }
            let key = self.name()?;
            self.skip_space();
            if !self.starts("=") {
                return Err(self.error("= expected after an attribute's name"));
            }
            self.at += 1;
            self.skip_space();
            let quote = match self.rest().first() {
                Some(&q @ (b'"' | b'\'')) => q,
                _ => return Err(self.error("a quoted attribute value expected")),
            };
            self.at += 1;
            let raw = self.until(if quote == b'"' { "\"" } else { "'" })?;
            let value = match raw.contains(['&', '\t', '\n', '\r']) {
                true => Cow::Owned(decode(raw).map_err(|why| self.error(&why))?),
                false => (self.keep)(raw),
            };
            element.attributes.push(((self.keep)(key), value));
        }
        self.children(&mut element, depth)?;
        Ok(element)
    }

    /// `element`'s children (`depth` its own) and its end tag.
    fn children(&mut self, element: &mut Element, depth: usize) -> Result<(), String> {
        loop {
            let start = self.at;
            self.skip_space();
            let lines = self.text[start..self.at]
                .iter()
                .filter(|&&b| b == b'\n')
                .count();
            element.children.extend((1..lines).map(|_| Child::Newline));
            if self.at >= self.text.len() {
                return Err(self.error(&format!("<{}> isn't closed", element.category)));
            }
            if self.starts("</") {
                self.at += 2;
                let name = self.name()?;
                self.skip_space();
                if name != element.category || !self.starts(">") {
                    return Err(self.error(&format!("</{}> expected", element.category)));
                }
                self.at += 1;
                return Ok(());
            }
            if self.starts("<!--") {
                self.at += 4;
                let text = (self.keep)(self.until("-->")?);
                element.children.push(Child::Comment(text));
            } else if self.starts("<?") {
                self.until("?>")?;
            } else if self.starts("<") {
                let child = self.element(depth + 1)?;
                element.children.push(Child::Element(child));
            } else {
                return Err(self.error("text inside an element"));
            }
        }
    }
}

/// An attribute's text decoded: entities and character references replaced,
/// tabs and line breaks as spaces (a `\r\n` as one).
fn decode(raw: &str) -> Result<String, String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find(['&', '\t', '\n', '\r']) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if let Some(after) = tail.strip_prefix("\r\n") {
            out.push(' ');
            rest = after;
            continue;
        }
        if !tail.starts_with('&') {
            out.push(' ');
            rest = &tail[1..];
            continue;
        }
        let end = tail
            .find(';')
            .ok_or_else(|| format!("an unterminated reference in {raw:?}"))?;
        let entity = &tail[1..end];
        let ch = match entity {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let code = match entity.strip_prefix("#x") {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => entity.strip_prefix('#').and_then(|d| d.parse().ok()),
                };
                code.and_then(char::from_u32)
                    .ok_or_else(|| format!("an unknown reference &{entity}; in {raw:?}"))?
            }
        };
        out.push(ch);
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_document_reads_and_writes_back_as_materialx_writes_it() {
        let text = "<?xml version=\"1.0\"?>\n<materialx version=\"1.39\">\n  <!-- a\n  comment -->\n  <nodedef name=\"ND_a\" node=\"a\">\n    <input name=\"in\" type=\"float\" value=\"1\" />\n  </nodedef>\n\n  <a name=\"x\" type=\"float\" doc=\"&lt;1 &amp; &quot;two&quot;\" />\n</materialx>\n";
        let root = parse(text).unwrap();
        assert_eq!(root.category, "materialx");
        assert_eq!(
            root.children.len(),
            4,
            "a comment, two elements, a blank line"
        );
        let a = root.elements().find(|e| e.category == "a").unwrap();
        assert_eq!(a.get("doc"), "<1 & \"two\"");
        // Angle brackets stay as they are, as MaterialX's pugixml writes them.
        assert_eq!(write(&root), text.replace("&lt;1", "<1"));
    }

    #[test]
    fn attributes_read_their_whitespace_as_spaces_and_references_as_characters() {
        let root = parse("<m a='x\n\ty' b=\"&#65;&#x42;\" />").unwrap();
        assert_eq!((root.get("a"), root.get("b")), ("x  y", "AB"));
        let mut out = String::new();
        escape("a\u{1}b", &mut out);
        assert_eq!(out, "a&#01;b");
    }

    #[test]
    fn nesting_past_its_depth_is_refused_on_a_small_stack() {
        let nested = |depth: usize| "<m>".repeat(depth) + &"</m>".repeat(depth);
        let read = std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || {
                let deepest = parse(&nested(DEPTH)).map(|root| write(&root).len());
                (deepest, parse(&nested(DEPTH + 1)), parse(&nested(100_000)))
            })
            .unwrap()
            .join()
            .unwrap();
        assert!(read.0.is_ok_and(|len| len > 0));
        for refused in [read.1, read.2] {
            assert!(
                refused
                    .unwrap_err()
                    .ends_with("elements nested deeper than 256")
            );
        }
    }

    #[test]
    fn broken_documents_are_refused_with_their_line() {
        for (text, line) in [
            ("<m>\n<a>\n</m>", 3),
            ("<m a=1 />", 1),
            ("<m>\ntext</m>", 2),
            ("<m />\n<n />", 2),
            ("<m a=\"&nope;\" />", 1),
        ] {
            let why = parse(text).unwrap_err();
            assert!(why.starts_with(&format!("line {line}:")), "{text:?}: {why}");
        }
    }
}
