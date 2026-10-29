//! A context's own user settings: its pages in the Settings window, their
//! defaults and its section of the user's settings file.
//!
//! A package declares a section as a value type implementing [`Section`]
//! and registers it ([`SettingsSection::of`],
//! [`UiPackage::settings`](super::UiPackage::settings)). The host keeps the
//! applied value and the Settings window's draft, draws each page from
//! [`Section::page`]'s sheet, applies the edits it reports through
//! [`Section::apply`], and saves the section with the rest of the user's
//! settings. A file holding no section for an installed package's context
//! does not read; a section of a package that isn't installed is skipped.
use crate::sheet::{Edits, Schema};
use kinogaki_studio_api::element::{ElementReader, ElementWriter};
use kinogaki_studio_api::{ContextId, Error};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// A context's settings section, as its value type.
pub trait Section: Any + Clone + PartialEq + fmt::Debug + Send + Sync {
    /// The context it belongs to (one section a context).
    fn context() -> ContextId;
    /// Its pages in the Settings window, in order.
    fn pages() -> Vec<String>;
    /// The values a new user starts with.
    fn defaults() -> Self;
    /// Page `page`'s rows, drawn by the host.
    fn page(&self, page: &str) -> Schema<u32>;
    /// The edits the host reported on page `page`'s rows.
    fn apply(&mut self, page: &str, rows: &Schema<u32>, edits: &Edits<u32>);
    /// Restores page `page`'s defaults (by default every value).
    fn reset(&mut self, page: &str) {
        let _ = page;
        *self = Self::defaults();
    }
    /// Writes the section's values.
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error>;
    /// Reads the section's values; fails on anything missing or unknown.
    fn read(from: &dyn ElementReader) -> Result<Self, Error>;
}

/// A shared section value.
type Value = Arc<dyn Any + Send + Sync>;

/// A registered section: its context and pages, and what the host does
/// with a value of its type without knowing the type. Made from the value
/// type with [`SettingsSection::of`]; cheap to copy.
#[derive(Clone, Copy)]
pub struct SettingsSection {
    context: fn() -> ContextId,
    pages: fn() -> Vec<String>,
    type_name: &'static str,
    defaults: fn() -> Value,
    copy: fn(&dyn Any) -> Value,
    eq: fn(&dyn Any, &dyn Any) -> bool,
    debug: fn(&dyn Any, &mut fmt::Formatter<'_>) -> fmt::Result,
    page: fn(&dyn Any, &str) -> Schema<u32>,
    apply: fn(&mut dyn Any, &str, &Schema<u32>, &Edits<u32>),
    reset: fn(&mut dyn Any, &str),
    write: fn(&dyn Any, &mut dyn ElementWriter) -> Result<(), Error>,
    read: fn(&dyn ElementReader) -> Result<Value, Error>,
}

fn typed<T: Section>(v: &dyn Any) -> &T {
    v.downcast_ref::<T>()
        .expect("a section value of its section's type")
}

fn typed_mut<T: Section>(v: &mut dyn Any) -> &mut T {
    v.downcast_mut::<T>()
        .expect("a section value of its section's type")
}

impl SettingsSection {
    /// The section `T` is.
    pub fn of<T: Section>() -> Self {
        SettingsSection {
            context: T::context,
            pages: T::pages,
            type_name: std::any::type_name::<T>(),
            defaults: || Arc::new(T::defaults()),
            copy: |v| Arc::new(typed::<T>(v).clone()),
            eq: |a, b| a.downcast_ref::<T>() == b.downcast_ref::<T>(),
            debug: |v, f| fmt::Debug::fmt(typed::<T>(v), f),
            page: |v, page| typed::<T>(v).page(page),
            apply: |v, page, rows, edits| typed_mut::<T>(v).apply(page, rows, edits),
            reset: |v, page| typed_mut::<T>(v).reset(page),
            write: |v, to| typed::<T>(v).write(to),
            read: |from| T::read(from).map(|v| Arc::new(v) as Value),
        }
    }

    /// The context it belongs to.
    pub fn context(&self) -> ContextId {
        (self.context)()
    }

    /// Its pages, in order.
    pub fn pages(&self) -> Vec<String> {
        (self.pages)()
    }

    /// Its default value.
    pub fn defaults(&self) -> Arc<dyn Any + Send + Sync> {
        (self.defaults)()
    }

    /// A copy of `value` (a draft to edit).
    pub fn copy(&self, value: &dyn Any) -> Arc<dyn Any + Send + Sync> {
        (self.copy)(value)
    }

    /// Whether `a` and `b` are equal values of the section.
    pub fn eq(&self, a: &dyn Any, b: &dyn Any) -> bool {
        (self.eq)(a, b)
    }

    /// Formats `value` for debugging.
    pub fn debug(&self, value: &dyn Any, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self.debug)(value, f)
    }

    /// Page `page`'s rows for `value`.
    pub fn page(&self, value: &dyn Any, page: &str) -> Schema<u32> {
        (self.page)(value, page)
    }

    /// Applies the edits of page `page`'s `rows` to `value`.
    pub fn apply(&self, value: &mut dyn Any, page: &str, rows: &Schema<u32>, edits: &Edits<u32>) {
        (self.apply)(value, page, rows, edits)
    }

    /// Restores page `page`'s defaults in `value`.
    pub fn reset(&self, value: &mut dyn Any, page: &str) {
        (self.reset)(value, page)
    }

    /// Writes `value`.
    pub fn write(&self, value: &dyn Any, to: &mut dyn ElementWriter) -> Result<(), Error> {
        (self.write)(value, to)
    }

    /// Reads a value.
    pub fn read(&self, from: &dyn ElementReader) -> Result<Arc<dyn Any + Send + Sync>, Error> {
        (self.read)(from)
    }
}

impl fmt::Debug for SettingsSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SettingsSection")
            .field("context", &self.context())
            .field("type", &self.type_name)
            .finish()
    }
}
