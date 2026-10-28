//! [`MemoryElement`]: the element traits over a plain in-memory tree, for
//! package tests (a codec's round trip without Studio).
use super::{ElementReader, ElementWriter, Field, FieldRef, ReadShared, SharedValue, check_name};
use crate::error::Error;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// The metadata key a reference to a shared child is kept under (never an
/// [`is_name`](super::is_name), so no codec's key can collide with it).
const SHARED: &str = "shared:";

/// An element held in memory: a kind, fields, metadata and children.
///
/// ```
/// use kinogaki_studio_api::element::{Field, MemoryElement};
///
/// let mut root = MemoryElement::new("cloud");
/// root.write(|w| w.set("count", Field::Int(3))).unwrap();
/// assert_eq!(root.read(|r| r.int("count")).unwrap(), 3);
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryElement {
    kind: String,
    fields: BTreeMap<String, Field>,
    metadata: BTreeMap<String, String>,
    children: Vec<(String, MemoryElement)>,
}

impl MemoryElement {
    /// An empty element of `kind`.
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            ..Self::default()
        }
    }

    /// Runs `write` with a writer of this element, as one save (shared
    /// values are shared within it).
    pub fn write<R>(
        &mut self,
        write: impl FnOnce(&mut dyn ElementWriter) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let mut shared = BTreeMap::new();
        let mut writer = Writer {
            element: self,
            path: String::new(),
            shared: &mut shared,
        };
        write(&mut writer)
    }

    /// Runs `read` with a reader of this element, as one load (each shared
    /// value is read once).
    pub fn read<R>(&self, read: impl FnOnce(&dyn ElementReader) -> R) -> R {
        let cache = RefCell::new(BTreeMap::new());
        let reader = Reader {
            root: self,
            element: self,
            path: String::new(),
            cache: &cache,
        };
        read(&reader)
    }

    /// The element's kind.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Child `name`, when there is one.
    pub fn child(&self, name: &str) -> Option<&MemoryElement> {
        self.children
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, e)| e)
    }

    /// The element at `path` (`/a/b`, relative to this one; `""` is this).
    fn at(&self, path: &str) -> Option<&MemoryElement> {
        path.split('/')
            .filter(|s| !s.is_empty())
            .try_fold(self, |e, name| e.child(name))
    }
}

/// Writes one [`MemoryElement`].
struct Writer<'a> {
    element: &'a mut MemoryElement,
    /// From the root, `/`-separated.
    path: String,
    /// Shared values written in this save: key → path.
    shared: &'a mut BTreeMap<u64, String>,
}

impl ElementWriter for Writer<'_> {
    fn set(&mut self, name: &str, field: Field) -> Result<(), Error> {
        check_name(name)?;
        self.element.fields.insert(name.to_owned(), field);
        Ok(())
    }

    fn set_rows(&mut self, name: &str, width: u32, field: Field) -> Result<(), Error> {
        if width == 0 || !field.len().is_multiple_of(width as usize) {
            return Err(Error::invalid(format_args!(
                "{}: {name} has {} values, not rows of {width}",
                self.path,
                field.len()
            )));
        }
        self.set(name, field)
    }

    fn set_metadata(&mut self, key: &str, value: &str) -> Result<(), Error> {
        check_name(key)?;
        self.element
            .metadata
            .insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    fn child(&mut self, name: &str, kind: &str) -> Result<Box<dyn ElementWriter + '_>, Error> {
        check_name(name)?;
        if self.element.child(name).is_some() {
            return Err(Error::invalid(format_args!(
                "{}: {name} is written twice",
                self.path
            )));
        }
        self.element
            .children
            .push((name.to_owned(), MemoryElement::new(kind)));
        let (_, element) = self.element.children.last_mut().expect("just pushed");
        Ok(Box::new(Writer {
            element,
            path: format!("{}/{name}", self.path),
            shared: &mut *self.shared,
        }))
    }

    fn share(
        &mut self,
        name: &str,
        kind: &str,
        key: u64,
        write: &mut dyn FnMut(&mut dyn ElementWriter) -> Result<(), Error>,
    ) -> Result<(), Error> {
        check_name(name)?;
        if let Some(at) = self.shared.get(&key) {
            let at = at.clone();
            self.element.metadata.insert(format!("{SHARED}{name}"), at);
            return Ok(());
        }
        let at = format!("{}/{name}", self.path);
        write(&mut *self.child(name, kind)?)?;
        self.shared.insert(key, at);
        Ok(())
    }
}

/// Reads one [`MemoryElement`].
struct Reader<'a> {
    root: &'a MemoryElement,
    element: &'a MemoryElement,
    path: String,
    /// Shared values read in this load, by path.
    cache: &'a RefCell<BTreeMap<String, SharedValue>>,
}

impl<'a> Reader<'a> {
    fn at(&self, element: &'a MemoryElement, path: String) -> Reader<'a> {
        Reader {
            root: self.root,
            element,
            path,
            cache: self.cache,
        }
    }
}

impl ElementReader for Reader<'_> {
    fn kind(&self) -> &str {
        &self.element.kind
    }

    fn field(&self, name: &str) -> Option<FieldRef<'_>> {
        self.element.fields.get(name).map(Field::as_ref)
    }

    fn metadata(&self, key: &str) -> Option<&str> {
        self.element.metadata.get(key).map(String::as_str)
    }

    fn children(&self) -> Vec<String> {
        self.element
            .children
            .iter()
            .map(|(n, _)| n.clone())
            .collect()
    }

    fn child(&self, name: &str) -> Option<Box<dyn ElementReader + '_>> {
        let element = self.element.child(name)?;
        Some(Box::new(self.at(element, format!("{}/{name}", self.path))))
    }

    fn shared_any(&self, name: &str, read: &mut ReadShared<'_>) -> Result<SharedValue, Error> {
        let path = match self.element.metadata.get(&format!("{SHARED}{name}")) {
            Some(at) => at.clone(),
            None => format!("{}/{name}", self.path),
        };
        if let Some(value) = self.cache.borrow().get(&path) {
            return Ok(value.clone());
        }
        let element = self
            .root
            .at(&path)
            .ok_or_else(|| Error::invalid(format_args!("{}: no shared {name}", self.path)))?;
        let value = read(&self.at(element, path.clone()))?;
        self.cache.borrow_mut().insert(path, value.clone());
        Ok(value)
    }

    fn location(&self) -> String {
        if self.path.is_empty() {
            "/".to_owned()
        } else {
            self.path.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn fields_children_and_metadata_round_trip() {
        let mut root = MemoryElement::new("cloud");
        root.write(|w| {
            w.set_rows("points", 3, Field::F64s(vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]))?;
            w.set_metadata("source", "scan")?;
            let mut part = w.child("part", "group")?;
            part.set("name", Field::Text("left".into()))
        })
        .unwrap();
        root.read(|r| {
            assert_eq!(r.kind(), "cloud");
            assert_eq!(r.f64s("points").unwrap().len(), 6);
            assert_eq!(r.metadata("source"), Some("scan"));
            assert_eq!(r.children(), ["part"]);
            let part = r.required_child("part").unwrap();
            assert_eq!((part.kind(), part.text("name").unwrap()), ("group", "left"));
            assert_eq!(part.location(), "/part");
            // Strict: a missing or mistyped field names itself.
            assert!(r.text("points").unwrap_err().to_string().contains("points"));
            assert!(r.int("nothing").is_err() && r.child("nothing").is_none());
        });
    }

    #[test]
    fn bad_names_and_rows_are_refused() {
        let mut root = MemoryElement::new("x");
        root.write(|w| {
            assert!(w.set("a.b", Field::Int(1)).is_err());
            assert!(w.set_rows("p", 3, Field::F32s(vec![0.0; 4])).is_err());
            w.child("c", "k")?;
            assert!(w.child("c", "k").is_err());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_shared_value_is_written_once_and_read_once() {
        let mut root = MemoryElement::new("project");
        let mut writes = 0;
        root.write(|w| {
            for place in ["a", "b"] {
                let mut holder = w.child(place, "holder")?;
                holder.share("mesh", "mesh", 7, &mut |m| {
                    writes += 1;
                    m.set("count", Field::Int(4))
                })?;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(writes, 1);
        let mut reads = 0;
        let (a, b) = root.read(|r| {
            let mut read = |name: &str| {
                let holder = r.required_child(name)?;
                holder.read_shared("mesh", |m| {
                    reads += 1;
                    m.int("count")
                })
            };
            (read("a").unwrap(), read("b").unwrap())
        });
        assert!(Arc::ptr_eq(&a, &b) && *a == 4 && reads == 1);
    }
}
