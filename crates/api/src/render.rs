//! Neutral render items: how any data type reaches every render engine
//! ([`RenderItem`]), made of [`TriangleSource`]s.
//!
//! Engines never see kernel types: a CAD body renders as its
//! tessellation's triangles, a mesh as its display triangulation. The
//! source contract ([`TriangleSource`], [`Ranges`], [`Look`],
//! [`MeshAttribute`], [`AttributeDomain`]) and its plain source
//! ([`PlainTriangles`]) are the render interface's
//! (`kinogaki-render-api`), the crate engines build on; they are re-exported
//! here, so a source a data type makes is what engines' hosts consume.
use crate::math::Mat4;
use std::fmt;
use std::sync::Arc;

pub use kinogaki_render_api::{
    AttributeDomain, ImportedMaterial, Look, MeshAttribute, PlainTriangles, Ranges, TriangleSource,
};

/// A file's materials as a format reads them beside its geometry
/// (`kinogaki.render.preview_materials`): what File ▸ Import turns into a
/// Material Library.
pub fn preview_materials() -> crate::Typed<Vec<ImportedMaterial>> {
    static ID: std::sync::LazyLock<crate::DataTypeId> = std::sync::LazyLock::new(|| {
        crate::DataTypeId::declared("kinogaki.render.preview_materials")
    });
    crate::Typed::new(*ID)
}

/// What a value renders as.
#[derive(Clone)]
pub enum RenderItem {
    /// Triangles placed once or many times.
    Triangles {
        /// A key stable across recooks for this item of its object (a CAD
        /// body's index): the host matches items of successive values by
        /// it.
        part: u32,
        /// The triangles.
        source: Arc<dyn TriangleSource>,
        /// Placements relative to the object, which the host places in the
        /// world: the first is the item itself, the rest are copies that
        /// share its acceleration structure. Empty: not rendered.
        placements: Arc<[Mat4]>,
        /// Its name (a body's), the name Cryptomatte gives the object.
        label: Option<Arc<str>>,
    },
}

impl RenderItem {
    /// `source` as part `part`, placed once where the object is.
    pub fn triangles(part: u32, source: Arc<dyn TriangleSource>) -> Self {
        Self::Triangles {
            part,
            source,
            placements: Arc::from([Mat4::IDENTITY]),
            label: None,
        }
    }
}

impl fmt::Debug for RenderItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Triangles {
                part,
                source,
                placements,
                label,
            } => write!(
                f,
                "Triangles(part {part}, {} × {}, {label:?})",
                source.triangle_count(),
                placements.len()
            ),
        }
    }
}
