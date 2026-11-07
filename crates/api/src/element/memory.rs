//! [`MemoryElement`]: the element traits over a plain in-memory tree, for
//! package tests (a codec's round trip without Studio). Its reads are as
//! strict as Studio's: what a read leaves unread is refused.
use super::{ElementReader, ElementWriter, Field, FieldRef, ReadShared, SharedValue, check_name};
use crate::Error;
use crate::data::Data;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

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
    /// Values written with [`ElementWriter::data`], kept as the values
    /// they are (memory needs no codec to hold them).
    values: Vec<(String, Value)>,
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
    /// value is read once), strictly as Studio reads a project: a field,
    /// metadata entry or child below this element that `read` didn't read
    /// is an error naming it, as anything missing is.
    pub fn read<R>(
        &self,
        read: impl FnOnce(&dyn ElementReader) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let cache = RefCell::new(BTreeMap::new());
        let taken = RefCell::new(BTreeMap::new());
        let reader = Reader {
            root: self,
            element: self,
            path: String::new(),
            cache: &cache,
            taken: &taken,
        };
        reader.visit();
        let value = read(&reader)?;
        match leftover(self, &taken.borrow(), &cache.borrow()) {
            Some(left) => Err(Error::invalid(left)),
            None => Ok(value),
        }
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

    /// Whether `other` holds what this element holds, each value
    /// [`ElementWriter::data`] wrote the same allocation or, of one type,
    /// the same as `same` says (Studio's registry: by content). `==`
    /// compares values by allocation alone.
    pub fn same_as(&self, other: &Self, same: &dyn Fn(&Data, &Data) -> bool) -> bool {
        let values = |(n, a): &(String, Value), (m, b): &(String, Value)| {
            n == m && (a.0.ptr_eq(&b.0) || a.0.ty() == b.0.ty() && same(&a.0, &b.0))
        };
        let children =
            |(n, a): &(String, Self), (m, b): &(String, Self)| n == m && a.same_as(b, same);
        self.kind == other.kind
            && self.fields == other.fields
            && self.metadata == other.metadata
            && self.values.len() == other.values.len()
            && (self.values.iter().zip(&other.values)).all(|(a, b)| values(a, b))
            && self.children.len() == other.children.len()
            && (self.children.iter().zip(&other.children)).all(|(a, b)| children(a, b))
    }

    /// The element at `path` (`/a/b`, relative to this one; `""` is this).
    fn at(&self, path: &str) -> Option<&MemoryElement> {
        path.split('/')
            .filter(|s| !s.is_empty())
            .try_fold(self, |e, name| e.child(name))
    }
}

/// Writes one [`MemoryElement`].
/// A value held by a memory element: equal when it is the same value.
#[derive(Clone)]
struct Value(Data);

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.ty())
    }
}

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

    fn data(&mut self, name: &str, data: &Data) -> Result<(), Error> {
        check_name(name)?;
        if self.element.values.iter().any(|(n, _)| n == name) {
            return Err(Error::invalid(format_args!(
                "{}: {name} is written twice",
                self.path
            )));
        }
        (self.element.values).push((name.to_owned(), Value(data.clone())));
        Ok(())
    }
}

/// What a read took of one element: its fields and metadata keys.
#[derive(Default)]
struct Taken {
    fields: BTreeSet<String>,
    metadata: BTreeSet<String>,
    values: BTreeSet<String>,
}

/// Reads one [`MemoryElement`].
struct Reader<'a> {
    root: &'a MemoryElement,
    element: &'a MemoryElement,
    path: String,
    /// Shared values read in this load, by path.
    cache: &'a RefCell<BTreeMap<String, SharedValue>>,
    /// What was read of each element visited, by path.
    taken: &'a RefCell<BTreeMap<String, Taken>>,
}

impl<'a> Reader<'a> {
    fn at(&self, element: &'a MemoryElement, path: String) -> Reader<'a> {
        let reader = Reader {
            root: self.root,
            element,
            path,
            cache: self.cache,
            taken: self.taken,
        };
        reader.visit();
        reader
    }

    fn visit(&self) {
        self.taken
            .borrow_mut()
            .entry(self.path.clone())
            .or_default();
    }

    fn take(&self, mark: impl FnOnce(&mut Taken)) {
        mark(
            self.taken
                .borrow_mut()
                .entry(self.path.clone())
                .or_default(),
        );
    }
}

/// The first thing of an element below `root` a read visited that it
/// didn't take, as `<element>: <what>` (Studio's rule and wording); a
/// child already read as a shared value counts as taken.
fn leftover(
    root: &MemoryElement,
    taken: &BTreeMap<String, Taken>,
    shared: &BTreeMap<String, SharedValue>,
) -> Option<String> {
    for (path, marks) in taken {
        let Some(e) = root.at(path) else {
            continue;
        };
        let at = if path.is_empty() { "/" } else { path };
        if let Some(name) = e.fields.keys().find(|n| !marks.fields.contains(*n)) {
            return Some(format!("{at}: unknown field {name:?}"));
        }
        if let Some(key) = e.metadata.keys().find(|k| !marks.metadata.contains(*k)) {
            return Some(format!("{at}: unknown metadata {key:?}"));
        }
        if let Some((name, _)) = e.values.iter().find(|(n, _)| !marks.values.contains(n)) {
            return Some(format!("{at}: unknown value {name:?}"));
        }
        for (name, child) in &e.children {
            let below = format!("{path}/{name}");
            if !taken.contains_key(&below) && !shared.contains_key(&below) {
                return Some(format!("{at}: unknown element {:?} {name:?}", child.kind));
            }
        }
    }
    None
}

impl ElementReader for Reader<'_> {
    fn kind(&self) -> &str {
        &self.element.kind
    }

    fn field(&self, name: &str) -> Option<FieldRef<'_>> {
        let field = self.element.fields.get(name)?;
        self.take(|t| _ = t.fields.insert(name.to_owned()));
        Some(field.as_ref())
    }

    fn fields(&self) -> Vec<String> {
        self.element.fields.keys().cloned().collect()
    }

    fn metadata(&self, key: &str) -> Option<&str> {
        let value = self.element.metadata.get(key)?;
        self.take(|t| _ = t.metadata.insert(key.to_owned()));
        Some(value)
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
        let path = match self.metadata(&format!("{SHARED}{name}")) {
            Some(at) => at.to_owned(),
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

    fn data(&self, name: &str) -> Option<Result<Data, Error>> {
        let (_, value) = self.element.values.iter().find(|(n, _)| n == name)?;
        self.take(|t| _ = t.values.insert(name.to_owned()));
        Some(Ok(value.0.clone()))
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
            Ok(())
        })
        .unwrap();
        // Strict: what a read leaves unread is refused, named.
        let left = |read: fn(&dyn ElementReader) -> Result<(), Error>| {
            root.read(read).unwrap_err().to_string()
        };
        assert!(left(|_| Ok(())).contains("/: unknown field \"points\""));
        let no_metadata = |r: &dyn ElementReader| r.f64s("points").map(drop);
        assert!(left(no_metadata).contains("/: unknown metadata \"source\""));
        fn no_child(r: &dyn ElementReader) -> Result<(), Error> {
            r.f64s("points")?;
            r.metadata("source");
            Ok(())
        }
        assert!(left(no_child).contains("/: unknown element \"group\" \"part\""));
        let no_name = |r: &dyn ElementReader| {
            no_child(r)?;
            r.required_child("part").map(drop)
        };
        assert!(left(no_name).contains("/part: unknown field \"name\""));
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
        let (a, b) = root
            .read(|r| {
                let mut read = |name: &str| {
                    let holder = r.required_child(name)?;
                    holder.read_shared("mesh", |m| {
                        reads += 1;
                        m.int("count")
                    })
                };
                Ok((read("a")?, read("b")?))
            })
            .unwrap();
        assert!(Arc::ptr_eq(&a, &b) && *a == 4 && reads == 1);
    }

    #[test]
    fn values_are_the_same_as_the_caller_says() {
        let ty = crate::DataTypeId::declared("example.counts.count");
        let holding = |n: i64| {
            let mut e = MemoryElement::new("holder");
            e.write(|w| w.data("value", &Data::new(ty, n))).unwrap();
            e
        };
        let (a, b, c) = (holding(4), holding(4), holding(5));
        assert!(a != b, "== compares values by allocation");
        let by_count = |x: &Data, y: &Data| x.downcast_ref::<i64>() == y.downcast_ref::<i64>();
        assert!(a.same_as(&b, &by_count) && !a.same_as(&c, &by_count));
        assert!(
            a.same_as(&a.clone(), &|_, _| false),
            "one allocation is the same"
        );
    }
}
