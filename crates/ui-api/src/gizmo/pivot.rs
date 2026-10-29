//! The transform pivot: the point the gizmo sits at and Rotate and Scale
//! turn and scale about. The host keeps the user's choice ([`Pivot`]) per
//! context and resolves it against the selection's [`PivotFrame`], which
//! the open context's [`Levels::pivot`](crate::package::Levels::pivot)
//! gives.
use kinogaki_studio_api::Vec3;

/// Where the transform pivot starts in a context (its library's
/// [`ViewportSpec::default_pivot`](crate::package::ViewportSpec::default_pivot)).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pivot {
    /// The selection's centre.
    #[default]
    Centre,
    /// The space's origin (each object's own, at an object level).
    Origin,
}

/// The selection as the pivot reads it, in its context's space (an
/// object's components: the object's; a sketch: its plane's, X and Y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PivotFrame {
    /// The selection's centre.
    pub centre: Vec3,
    /// The space's origin.
    pub origin: Vec3,
    /// The selection's box along the space's axes: its least corner.
    pub min: Vec3,
    /// Its greatest corner.
    pub max: Vec3,
    /// The axes the box's points offer: 3, or 2 in a plane (X and Y).
    pub axes: usize,
    /// Whether the space has an origin to offer.
    pub has_origin: bool,
}
