//! Neutral render items: how any data type reaches every render engine
//! ([`RenderItem`]), made of [`ShapeSource`]s and instances of other items.
//!
//! Engines never see kernel types: a CAD body renders as its
//! tessellation's triangles, a polygon value as its display triangulation and
//! its curves, instances as an Instancer of their prototypes' items. The
//! source contract ([`ShapeSource`], [`Ranges`], [`Topology`]) and its plain
//! source ([`PlainTriangles`]) are the render interface's
//! (`kinogaki-render-api`), the crate engines build on; they are re-exported
//! here, so a source a data type makes is what engines' hosts consume.
//!
//! An engine's settings and its declared prims' values are parameters of
//! the one value model ([`Parameter::setting`](crate::params::Parameter::setting)),
//! keyed over time as node parameters are. A Material's nodes are engine
//! declared ([`ShaderNode`]s over `NodeDef`s).
mod declared;
mod light_view;
mod shader;

pub use declared::DeclaredKind;
pub use shader::{SHADER, SHADING, ShaderNode};

use kinogaki_geometry::{Attributes, Column, Xform};
use std::fmt;
use std::sync::Arc;

pub use kinogaki_render_api::{Detail, PlainTriangles, Ranges, ShapeSource, Topology};

/// What a value renders as.
#[derive(Clone)]
#[non_exhaustive]
pub enum RenderItem {
    /// Triangles or curves, where the object is.
    Shape {
        /// A key stable across recooks for this item of its object (a CAD
        /// body's index): the host matches items of successive values by
        /// it.
        part: u32,
        /// The shape.
        source: Arc<dyn ShapeSource>,
        /// Its name (a body's), the name Cryptomatte gives the object.
        label: Option<Arc<str>>,
    },
    /// Placements of other items: an engine instances them, sharing what it
    /// made of each prototype, however many instances place it.
    Instances {
        /// As a shape's.
        part: u32,
        /// What each prototype renders as. An item's own parts number within
        /// its prototype.
        prototypes: Arc<[Arc<[RenderItem]>]>,
        /// Each instance's index into the prototypes; none: all the first.
        which: Option<Column<u32>>,
        /// Each instance's placement, relative to the object.
        xforms: Column<Xform>,
        /// What differs between instances: their Instance and Detail
        /// attributes.
        primvars: Arc<Attributes>,
        /// As a shape's.
        label: Option<Arc<str>>,
    },
}

impl RenderItem {
    /// `source` as part `part`, where the object is.
    pub fn shape(part: u32, source: Arc<dyn ShapeSource>) -> Self {
        Self::Shape {
            part,
            source,
            label: None,
        }
    }

    /// The key it is matched by across recooks.
    pub fn part(&self) -> u32 {
        match self {
            Self::Shape { part, .. } | Self::Instances { part, .. } => *part,
        }
    }
}

impl fmt::Debug for RenderItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape {
                part,
                source,
                label,
            } => {
                let primitives = source.topology().primitives();
                write!(f, "Shape(part {part}, {primitives}, {label:?})")
            }
            Self::Instances {
                part,
                prototypes,
                xforms,
                label,
                ..
            } => write!(
                f,
                "Instances(part {part}, {} × {}, {label:?})",
                prototypes.len(),
                xforms.len()
            ),
        }
    }
}
