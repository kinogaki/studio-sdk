//! [`Pickable`]: turning the viewport's picks into component selections.
use crate::components::Components;
use crate::data::Data;
use crate::facets::Drawn;
use crate::viewport::{ElementKind, ElementSet, Hit};

/// How a new selection meets the current one (click, Shift, Ctrl).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Combine {
    /// The new selection replaces the current one.
    Replace,
    /// Both together.
    Add,
    /// The current one without the new one.
    Remove,
    /// Each new component flips.
    Toggle,
}

/// How viewport picks on a [`Drawn`](crate::Drawn) surface become
/// [`Components`] of the value. The viewport finds surface elements
/// (triangles, segments and markers, by the ids the surface gave them);
/// this facet says which components of the value they are.
pub trait Pickable: Send + Sync {
    /// What `hit` on `drawn` selects at `level`; `None` when it hit
    /// nothing selectable at that level.
    fn pick(&self, data: &Data, drawn: &Drawn, hit: &Hit, level: ElementKind)
    -> Option<Components>;

    /// The components a set of surface elements (a box or lasso's) is.
    fn select(&self, data: &Data, drawn: &Drawn, elements: &ElementSet) -> Components;

    /// `current` and `new` (both of `data`, at one level) combined by
    /// `how`.
    fn combine(
        &self,
        data: &Data,
        current: &Components,
        new: &Components,
        how: Combine,
    ) -> Components;
}
