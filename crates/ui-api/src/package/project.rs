//! A package's sections of Settings ▸ Project: rows over the project's
//! stored values (the standard ones core reads, such as
//! [`ColorConfiguration`](kinogaki_studio_api::standard::ColorConfiguration),
//! and the package's own of [`Scope::Project`](kinogaki_studio_api::Scope)).
//!
//! A package declares a section as a type implementing [`ProjectSection`]
//! and registers it ([`ProjectSettingsSection::of`],
//! [`UiPackage::project_sections`](super::UiPackage::project_sections)).
//! The page shows only while a document is open. Like the rest of the
//! Settings window it edits a draft of the project's values; OK or Apply
//! puts every change of the page into the document as one edit (one undo
//! step), and Cancel drops them.
use crate::host::Cx;
use crate::sheet::{Edits, Schema};
use kinogaki_studio_api::StoredValues;
use std::any::Any;
use std::marker::PhantomData;

/// A section of the project's settings.
pub trait ProjectSection: 'static {
    /// Its package's UI state ([`UiPackage::State`](super::UiPackage::State)),
    /// lent to it.
    type State: Any;
    /// What the page heads it ("Color").
    fn label() -> String;
    /// Its rows over the draft of the project's values `project`.
    fn rows(project: &StoredValues, s: &mut Self::State, cx: &mut dyn Cx) -> Schema<u32>;
    /// The edits the host reported on its rows, written into the draft
    /// `project`.
    fn apply(
        project: &mut StoredValues,
        rows: &Schema<u32>,
        edits: &Edits<u32>,
        s: &mut Self::State,
        cx: &mut dyn Cx,
    );
}

/// [`ProjectSection::rows`] over the erased state.
type RowsFn = fn(&StoredValues, &mut dyn Any, &mut dyn Cx) -> Schema<u32>;
/// [`ProjectSection::apply`] over the erased state.
type ApplyFn = fn(&mut StoredValues, &Schema<u32>, &Edits<u32>, &mut dyn Any, &mut dyn Cx);

/// A registered project section, made from its type with
/// [`ProjectSettingsSection::of`] over its package's state `S`; the host
/// holds it over the erased state (`dyn Any`). Cheap to copy.
pub struct ProjectSettingsSection<S: ?Sized = dyn Any> {
    label: fn() -> String,
    type_name: &'static str,
    rows: RowsFn,
    apply: ApplyFn,
    state: PhantomData<fn() -> Box<S>>,
}

impl<S: ?Sized> Clone for ProjectSettingsSection<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: ?Sized> Copy for ProjectSettingsSection<S> {}

impl<S: ?Sized> std::fmt::Debug for ProjectSettingsSection<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (f.debug_struct("ProjectSettingsSection"))
            .field("label", &(self.label)())
            .field("type", &self.type_name)
            .finish()
    }
}

fn state<T: ProjectSection>(s: &mut dyn Any) -> &mut T::State {
    s.downcast_mut()
        .expect("the host lends each section its package's state")
}

impl<S: 'static> ProjectSettingsSection<S> {
    /// The section `T` is.
    pub fn of<T: ProjectSection<State = S>>() -> Self {
        ProjectSettingsSection {
            label: T::label,
            type_name: std::any::type_name::<T>(),
            rows: |project, s, cx| T::rows(project, state::<T>(s), cx),
            apply: |project, rows, edits, s, cx| T::apply(project, rows, edits, state::<T>(s), cx),
            state: PhantomData,
        }
    }

    /// The same section over the host's erased state.
    pub fn erase(self) -> ProjectSettingsSection {
        let Self {
            label,
            type_name,
            rows,
            apply,
            state: _,
        } = self;
        ProjectSettingsSection {
            label,
            type_name,
            rows,
            apply,
            state: PhantomData,
        }
    }
}

impl<S: ?Sized> ProjectSettingsSection<S> {
    /// What the page heads it.
    pub fn label(&self) -> String {
        (self.label)()
    }

    /// Its rows over `project`, lent the package's `state`.
    pub fn rows(
        &self,
        project: &StoredValues,
        state: &mut dyn Any,
        cx: &mut dyn Cx,
    ) -> Schema<u32> {
        (self.rows)(project, state, cx)
    }

    /// `edits` on `rows`, written into `project`.
    pub fn apply(
        &self,
        project: &mut StoredValues,
        (rows, edits): (&Schema<u32>, &Edits<u32>),
        state: &mut dyn Any,
        cx: &mut dyn Cx,
    ) {
        (self.apply)(project, rows, edits, state, cx)
    }
}
