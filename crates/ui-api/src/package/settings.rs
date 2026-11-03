//! A context's own user settings: its pages in the Settings window, their
//! defaults and its section of the user's settings file.
//!
//! A package declares a section as a value type implementing [`Section`]
//! and registers it ([`SettingsSection::of`],
//! [`UiPackage::settings`](super::UiPackage::settings)). The host keeps the
//! applied value and the Settings window's draft, draws each page from
//! [`Section::page`]'s sheet, applies the edits it reports through
//! [`Section::apply`], and saves the section with the rest of the user's
//! settings. Both are lent the package's UI state and the call's context,
//! as parts are, so a page can show what is live (a device's status, the
//! printers on the network) and start what it shows. An installed package without a section starts from its
//! defaults; a section of a package that isn't installed is kept and
//! written back unchanged.
use crate::host::Cx;
use crate::sheet::{Edits, Schema};
use kinogaki_studio_api::element::{ElementReader, ElementWriter};
use kinogaki_studio_api::{ContextId, Error};
use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// A context's settings section, as its value type.
pub trait Section: Any + Clone + PartialEq + fmt::Debug + Send + Sync {
    /// Its package's UI state ([`UiPackage::State`](super::UiPackage::State)),
    /// lent to its pages.
    type State: Any;
    /// The context it belongs to (one section a context).
    fn context() -> ContextId;
    /// Its pages in the Settings window, in order.
    fn pages() -> Vec<String>;
    /// The values a new user starts with.
    fn defaults() -> Self;
    /// Page `page`'s rows, drawn by the host while the page shows.
    fn page(&self, page: &str, s: &mut Self::State, cx: &mut dyn Cx) -> Schema<u32>;
    /// The edits the host reported on page `page`'s rows.
    fn apply(
        &mut self,
        page: &str,
        rows: &Schema<u32>,
        edits: &Edits<u32>,
        s: &mut Self::State,
        cx: &mut dyn Cx,
    );
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

/// [`Section::apply`] over the erased value and state.
type ApplyFn = fn(&mut dyn Any, &str, &Schema<u32>, &Edits<u32>, &mut dyn Any, &mut dyn Cx);

/// A registered section: its context and pages, and what the host does
/// with a value of its type without knowing the type. Made from the value
/// type with [`SettingsSection::of`] over its package's state `S`; the
/// host holds it over the erased state (`dyn Any`). Cheap to copy.
pub struct SettingsSection<S: ?Sized = dyn Any> {
    context: fn() -> ContextId,
    pages: fn() -> Vec<String>,
    type_name: &'static str,
    defaults: fn() -> Value,
    copy: fn(&dyn Any) -> Box<dyn Any + Send + Sync>,
    eq: fn(&dyn Any, &dyn Any) -> bool,
    debug: fn(&dyn Any, &mut fmt::Formatter<'_>) -> fmt::Result,
    page: fn(&dyn Any, &str, &mut dyn Any, &mut dyn Cx) -> Schema<u32>,
    apply: ApplyFn,
    reset: fn(&mut dyn Any, &str),
    write: fn(&dyn Any, &mut dyn ElementWriter) -> Result<(), Error>,
    read: fn(&dyn ElementReader) -> Result<Value, Error>,
    state: PhantomData<fn() -> Box<S>>,
}

impl<S: ?Sized> Clone for SettingsSection<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: ?Sized> Copy for SettingsSection<S> {}

fn typed<T: Section>(v: &dyn Any) -> &T {
    v.downcast_ref::<T>()
        .expect("a section value of its section's type")
}

fn typed_mut<T: Section>(v: &mut dyn Any) -> &mut T {
    v.downcast_mut::<T>()
        .expect("a section value of its section's type")
}

fn state<T: Section>(s: &mut dyn Any) -> &mut T::State {
    s.downcast_mut()
        .expect("the host lends each section its package's state")
}

impl<S: 'static> SettingsSection<S> {
    /// The section `T` is.
    pub fn of<T: Section<State = S>>() -> Self {
        SettingsSection {
            context: T::context,
            pages: T::pages,
            type_name: std::any::type_name::<T>(),
            defaults: || Arc::new(T::defaults()),
            copy: |v| Box::new(typed::<T>(v).clone()),
            eq: |a, b| a.downcast_ref::<T>() == b.downcast_ref::<T>(),
            debug: |v, f| fmt::Debug::fmt(typed::<T>(v), f),
            page: |v, page, s, cx| typed::<T>(v).page(page, state::<T>(s), cx),
            apply: |v, page, rows, edits, s, cx| {
                typed_mut::<T>(v).apply(page, rows, edits, state::<T>(s), cx)
            },
            reset: |v, page| typed_mut::<T>(v).reset(page),
            write: |v, to| typed::<T>(v).write(to),
            read: |from| T::read(from).map(|v| Arc::new(v) as Value),
            state: PhantomData,
        }
    }

    /// The same section over the host's erased state.
    pub fn erase(self) -> SettingsSection {
        let Self {
            context,
            pages,
            type_name,
            defaults,
            copy,
            eq,
            debug,
            page,
            apply,
            reset,
            write,
            read,
            state: _,
        } = self;
        SettingsSection {
            context,
            pages,
            type_name,
            defaults,
            copy,
            eq,
            debug,
            page,
            apply,
            reset,
            write,
            read,
            state: PhantomData,
        }
    }
}

impl<S: ?Sized> SettingsSection<S> {
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

    /// A copy of `value` of its own (a draft to edit, then share).
    pub fn copy(&self, value: &dyn Any) -> Box<dyn Any + Send + Sync> {
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

    /// Page `page`'s rows for `value`, lent the package's `state`.
    pub fn page(
        &self,
        value: &dyn Any,
        page: &str,
        state: &mut dyn Any,
        cx: &mut dyn Cx,
    ) -> Schema<u32> {
        (self.page)(value, page, state, cx)
    }

    /// Applies the edits of page `page`'s `rows` to `value`, lent the
    /// package's `state`.
    pub fn apply(
        &self,
        value: &mut dyn Any,
        (page, rows, edits): (&str, &Schema<u32>, &Edits<u32>),
        state: &mut dyn Any,
        cx: &mut dyn Cx,
    ) {
        (self.apply)(value, page, rows, edits, state, cx)
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

impl<S: ?Sized> fmt::Debug for SettingsSection<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SettingsSection")
            .field("context", &self.context())
            .field("type", &self.type_name)
            .finish()
    }
}
