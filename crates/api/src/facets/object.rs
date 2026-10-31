//! [`ObjectPicking`] and [`Look`]: what the viewport asks of an object's
//! result at the object levels beyond its pieces. The first covers where a
//! ray meets it, whether a marquee holds it and its world box. The second
//! covers how it looks against the scene's [`Materials`].
use crate::data::Data;
use crate::facets::Drawn;
use crate::math::Bounds;
use kinogaki_render_api::Tint;
use std::sync::Arc;

/// A column-major 4×4 matrix of `f64` (the viewport's precision): an
/// object's world placement, or a view's world-to-clip.
pub type Matrix = [[f64; 4]; 4];

/// A world ray: from `origin` along `direction`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldRay {
    /// Where it starts.
    pub origin: [f64; 3],
    /// Where it goes (not necessarily of unit length).
    pub direction: [f64; 3],
}

/// How an object holding a value of the type is picked and framed at the
/// object levels. Without it, core frames the value by
/// [`DataType::bounds`](crate::DataType::bounds) and doesn't pick it.
pub trait ObjectPicking: Send + Sync {
    /// The world distance along `ray` to the nearest surface of `data`
    /// (drawn as `drawn`, placed at `world`) that the ray meets.
    fn ray(&self, data: &Data, drawn: &Drawn, world: &Matrix, ray: &WorldRay) -> Option<f64>;

    /// Whether a point of it lies in the box from `min` to `max` of the
    /// view whose world-to-clip is `view`. The box runs 0–1 across the
    /// view, down from its top.
    fn in_box(
        &self,
        data: &Data,
        drawn: &Drawn,
        view: &Matrix,
        world: &Matrix,
        min: [f64; 2],
        max: [f64; 2],
    ) -> bool;

    /// Its world box.
    fn bounds(&self, data: &Data, drawn: &Drawn, world: &Matrix) -> Option<Bounds>;

    /// The distance along `ray` to piece `piece` of `drawn` alone (a
    /// templated branch's packet), as [`ray`](Self::ray) measures the
    /// whole: what branch picks pick. `None`: missed, or the type doesn't
    /// pick pieces alone.
    fn piece_ray(
        &self,
        data: &Data,
        drawn: &Drawn,
        piece: usize,
        world: &Matrix,
        ray: &WorldRay,
    ) -> Option<f64> {
        let _ = (data, drawn, piece, world, ray);
        None
    }
}

/// What the scene's materials say, as an object's look reads them. It is
/// core's service, the same for every type.
pub trait Materials {
    /// The tint material `path` shows while shaded: its unwired base
    /// color, the attribute an Attribute node wired into it reads, or
    /// both. `None` when the path names no material or the material has
    /// no tint.
    fn tint(&self, path: &str) -> Option<Tint>;

    /// Whether material `path` fills its object as a volume.
    fn volume(&self, path: &str) -> bool;

    /// A number that changes whenever any answer may change (an edited
    /// library, another color management). Looks cache by it.
    fn revision(&self) -> u64;
}

/// How a value looks against the scene: its pieces' colors while shaded
/// and whether it renders as a volume. Without it, the value shows as
/// clay.
pub trait Look: Send + Sync {
    /// The linear colors of piece `piece` of `drawn`, packed as RGB9E5,
    /// one per polygon or per polygon vertex as the piece's packet lays
    /// them out. `None`: the clay.
    fn colors(
        &self,
        data: &Data,
        drawn: &Drawn,
        piece: usize,
        materials: &dyn Materials,
    ) -> Option<Arc<[u32]>>;

    /// Whether it renders as a volume (its surface only the volume's
    /// bounds, drawn as a ghost).
    fn volume(&self, data: &Data, materials: &dyn Materials) -> bool;
}
