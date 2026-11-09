//! [`Over`]: an element read over the defaults its codec writes, so what
//! the element lacks of a value reads as a new value holds it.
use super::{ElementReader, FieldRef, MemoryElement, ReadShared, SharedValue};
use crate::Error;
use crate::data::Data;

/// An element read over `defaults` ([`over`](trait.ElementReader.html#method.over)).
pub(super) struct Over<'a> {
    file: File<'a>,
    defaults: &'a MemoryElement,
}

/// The element an [`Over`] reads.
enum File<'a> {
    /// The element read.
    Read(&'a dyn ElementReader),
    /// A child of one.
    Child(Box<dyn ElementReader + 'a>),
    /// A child the element lacks, read from the defaults alone (said once,
    /// as the child), where it would be.
    Lacked(String),
}

impl<'a> Over<'a> {
    pub(super) fn new(file: &'a dyn ElementReader, defaults: &'a MemoryElement) -> Self {
        Self {
            file: File::Read(file),
            defaults,
        }
    }

    /// The element read, unless it is a child the file lacks.
    fn file(&self) -> Option<&dyn ElementReader> {
        match &self.file {
            File::Read(file) => Some(*file),
            File::Child(file) => Some(&**file),
            File::Lacked(_) => None,
        }
    }

    /// Child `name` of the defaults alone, where the element's would be.
    fn lacked(&self, name: &str, defaults: &'a MemoryElement) -> Over<'a> {
        Over {
            file: File::Lacked(format!("{}/{name}", self.location().trim_end_matches('/'))),
            defaults,
        }
    }
}

impl ElementReader for Over<'_> {
    fn kind(&self) -> &str {
        self.file().map_or(self.defaults.kind(), |f| f.kind())
    }

    fn field(&self, name: &str) -> Option<FieldRef<'_>> {
        let default = || self.defaults.field(name).map(|f| f.as_ref());
        let Some(file) = self.file() else {
            return default();
        };
        if let Some(field) = file.field(name) {
            return Some(field);
        }
        let default = default()?;
        file.defaulted(name, Some(default));
        Some(default)
    }

    fn fields(&self) -> Vec<String> {
        self.file()
            .map_or_else(|| self.defaults.field_names(), |f| f.fields())
    }

    fn metadata(&self, key: &str) -> Option<&str> {
        match self.file() {
            Some(file) => file.metadata(key),
            None => self.defaults.metadata(key),
        }
    }

    fn children(&self) -> Vec<String> {
        self.file()
            .map_or_else(|| self.defaults.child_names(), |f| f.children())
    }

    fn child(&self, name: &str) -> Option<Box<dyn ElementReader + '_>> {
        let default = self.defaults.child(name);
        let Some(file) = self.file() else {
            return default.map(|d| Box::new(self.lacked(name, d)) as Box<dyn ElementReader>);
        };
        match (file.child(name), default) {
            (Some(child), Some(defaults)) => Some(Box::new(Over {
                file: File::Child(child),
                defaults,
            })),
            (Some(child), None) => Some(child),
            (None, Some(defaults)) => {
                file.defaulted(name, None);
                Some(Box::new(self.lacked(name, defaults)))
            }
            (None, None) => None,
        }
    }

    /// The element's; of a child the file lacks, the default's child read
    /// as it is.
    fn shared_any(&self, name: &str, read: &mut ReadShared<'_>) -> Result<SharedValue, Error> {
        if let Some(file) = self.file() {
            return file.shared_any(name, read);
        }
        match self.defaults.child(name) {
            Some(defaults) => read(&self.lacked(name, defaults)),
            None => Err(Error::invalid(format_args!(
                "{}: no shared {name}",
                self.location()
            ))),
        }
    }

    fn data(&self, name: &str) -> Option<Result<Data, Error>> {
        let default = || self.defaults.value(name).cloned();
        let Some(file) = self.file() else {
            return default().map(Ok);
        };
        if let Some(data) = file.data(name) {
            return Some(data);
        }
        let default = default()?;
        file.defaulted(name, None);
        Some(Ok(default))
    }

    fn location(&self) -> String {
        match &self.file {
            File::Read(file) => file.location(),
            File::Child(file) => file.location(),
            File::Lacked(at) => at.clone(),
        }
    }

    fn defaulted(&self, name: &str, default: Option<FieldRef<'_>>) {
        if let Some(file) = self.file() {
            file.defaulted(name, default);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::element::{ElementReader, Field, MemoryElement};

    /// A default's child the element lacks is said once and read whole;
    /// one both hold is read over the default's, field by field.
    #[test]
    fn a_child_the_element_lacks_is_said_once_and_read_from_the_defaults() {
        let mut defaults = MemoryElement::new("value");
        defaults
            .write(|w| {
                let mut range = w.child("range", "range")?;
                range.set("low", Field::Float(0.0))?;
                range.set("high", Field::Float(1.0))?;
                drop(range);
                let mut look = w.child("look", "look")?;
                look.set("tint", Field::F64s(vec![1.0; 3]))?;
                look.set("on", Field::Bool(true))
            })
            .unwrap();
        let mut older = MemoryElement::new("value");
        older
            .write(|w| w.child("range", "range")?.set("low", Field::Float(0.5)))
            .unwrap();
        let read = |r: &dyn ElementReader| {
            let r = r.over(&defaults);
            let range = r.required_child("range")?;
            let look = r.required_child("look")?;
            assert_eq!((look.kind(), look.location()), ("look", "/look".into()));
            let tint = look.f64s("tint")?.to_vec();
            Ok((
                range.float("low")?,
                range.float("high")?,
                tint,
                look.bool("on")?,
            ))
        };
        let (read, said) = older.read_defaulted(read).unwrap();
        assert_eq!(read, (0.5, 1.0, vec![1.0; 3], true));
        assert_eq!(said, ["/: look", "/range: high"]);
        // What neither holds is missing, as without defaults.
        let none = older.read(|r| {
            let r = r.over(&defaults);
            r.required_child("range")?.float("low")?;
            r.float("width")
        });
        assert!(none.unwrap_err().to_string().contains("no field width"));
    }
}
