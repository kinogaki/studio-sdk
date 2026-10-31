//! How fine a value's geometry is made for rendering: a CAD body's
//! tessellation, a sampled surface, a subdivision level later.

/// How fine to make geometry for a render.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum Detail {
    /// As fine as the viewport draws it: interactive views and drags.
    #[default]
    Viewport,
    /// For a final render.
    Render {
        /// How far a facet may stray from the exact surface (scene units).
        chord: f64,
        /// How sharply neighboring facets may turn (degrees).
        angle: f64,
    },
}
