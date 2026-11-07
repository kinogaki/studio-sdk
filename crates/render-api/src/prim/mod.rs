//! The prims a host sends: shapes (meshes and curves), instancers, cameras,
//! materials and textures (the structural prims every scene is made of; a
//! texture is the SDK's one picture, an image with its decoding:
//! [`TextureImage`]), and
//! prims of the types an engine declares (lights, fog: [`crate::declared`]).
//! Payloads are shared columns, so sending a prim again costs a few reference
//! counts, never a copy of its arrays. A prim is made by its builder, which
//! checks it whole: an engine never sees one half-formed.
//!
//! Placements are [`Xform`](crate::geometry::Xform)s across the frame's
//! shutter ([`Sampled`](crate::geometry::Sampled): one sample for a prim that
//! doesn't move), in the SDK's one convention; an engine converts them to its
//! own single-precision layout where it uploads them. Positions and lengths
//! are in scene units (the [`SceneSettings`](crate::SceneSettings)' units
//! say what one is); camera lens and film back are in millimetres.
mod camera;
mod instancer;
mod shape;

pub use camera::{Camera, Projection};
pub use instancer::{Instancer, InstancerBuilder, Map, MaterialMaps};
pub use shape::{STANDARD_PRIMVARS, Shape, ShapeBuilder, Subsets, Topology};

use crate::declared::Declared;
use crate::geometry::image::TextureImage;
use crate::material::MaterialNetwork;
use std::sync::Arc;

/// Whether a prim is in the render, and what sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visibility {
    /// In the render at all; off, the prim does nothing.
    pub visible: bool,
    /// Seen by camera rays; off, it still lights, shadows and reflects
    /// (Houdini's phantom).
    pub camera: bool,
}

impl Visibility {
    /// In the render and seen by the camera.
    pub const SHOWN: Visibility = Visibility {
        visible: true,
        camera: true,
    };
    /// Out of the render.
    pub const HIDDEN: Visibility = Visibility {
        visible: false,
        camera: false,
    };

    /// In the render when `visible`, then seen by the camera.
    pub const fn of(visible: bool) -> Visibility {
        Visibility {
            visible,
            camera: visible,
        }
    }
}

/// A material prim.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    /// Its network.
    pub network: Arc<MaterialNetwork>,
    /// The host's path of the material: the name Cryptomatte gives it.
    pub path: Arc<str>,
    /// Whether both sides of a surface show it (MaterialX has no single
    /// sidedness; a Material says): a path tracer renders both sides
    /// whatever it says, a realtime engine culls back faces when it doesn't.
    pub double_sided: bool,
}

/// A prim's payload.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Prim {
    /// Triangles or curves in their own space.
    Shape(Shape),
    /// Placements of other prims.
    Instancer(Instancer),
    /// A camera.
    Camera(Camera),
    /// A prim of a type the engine declares (a light, fog).
    Declared(Declared),
    /// A material.
    Material(Material),
    /// A texture: the image a material node or a light reads, as the
    /// host's image files hold the file: its first layer the texture, in the
    /// file's own samples and 1 to 4 channels, over the image's data window
    /// (its display window). A color layer's colors reach linear ACEScg
    /// ([`RENDER_SPACE`]) through the texture's decoding, which the engine
    /// runs where it likes (on its device after fetching texels, or as it
    /// uploads them); premultiplied when its encoding says so. Data
    /// ([`Encoding::Raw`]) is read as stored. The engine picks the form it
    /// holds it in.
    ///
    /// [`RENDER_SPACE`]: crate::RENDER_SPACE
    /// [`Encoding::Raw`]: crate::geometry::image::Encoding::Raw
    Texture(TextureImage),
}

/// The kinds of prim: the structural ones, for what an engine supports
/// ([`crate::Capabilities::prims`]), and declared prims, which an engine
/// takes of the types it declares.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PrimKind {
    Triangles,
    Curves,
    Instancer,
    Camera,
    Material,
    Texture,
    Declared,
}

impl PrimKind {
    /// The structural kinds.
    pub(crate) const STRUCTURAL: [PrimKind; 6] = [
        PrimKind::Triangles,
        PrimKind::Curves,
        PrimKind::Instancer,
        PrimKind::Camera,
        PrimKind::Material,
        PrimKind::Texture,
    ];
}

impl Prim {
    /// The prim's kind.
    pub fn kind(&self) -> PrimKind {
        match self {
            Prim::Shape(shape) => shape.topology().kind(),
            Prim::Instancer(_) => PrimKind::Instancer,
            Prim::Camera(_) => PrimKind::Camera,
            Prim::Declared(_) => PrimKind::Declared,
            Prim::Material(_) => PrimKind::Material,
            Prim::Texture(_) => PrimKind::Texture,
        }
    }
}
