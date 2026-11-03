//! The prims a host sends: meshes, instancers, cameras, materials and
//! textures (the structural prims every scene is made of), and prims of the
//! types an engine declares (lights, fog: [`crate::declared`]). Payloads are `Arc` data, so sending a prim again costs a few
//! reference counts, never a copy of its arrays.
//!
//! Matrices are column-major `[[f32; 4]; 4]` (glam's `to_cols_array_2d`):
//! `transform[3]` is the translation. Positions and lengths are in scene
//! units; camera lens and film back are in millimetres.

use std::sync::Arc;

use crate::declared::Declared;
use crate::id::{MaterialId, PrimId};
use crate::material::MaterialNetwork;

/// A column-major 4×4 matrix.
pub type Matrix = [[f32; 4]; 4];

/// The identity matrix.
pub const IDENTITY: Matrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// A prim's placements across the shutter, for motion blur: `values[k]`
/// at `times[k]`, in frames relative to the frame rendered, ascending.
/// Empty: it doesn't move, its rest placement holding all the shutter. A
/// host sends samples only to an engine whose
/// [`MotionSupport`](crate::MotionSupport) takes them.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion<T> {
    /// The sample times.
    pub times: Arc<[f32]>,
    /// One placement a time.
    pub values: Arc<[T]>,
}

impl<T> Motion<T> {
    /// No motion.
    pub fn none() -> Motion<T> {
        Motion {
            times: Arc::from([]),
            values: Arc::from([]),
        }
    }

    /// `values` at `times`, when there is one a time and the times are
    /// finite and ascending.
    pub fn new(times: impl Into<Arc<[f32]>>, values: impl Into<Arc<[T]>>) -> Option<Motion<T>> {
        let (times, values) = (times.into(), values.into());
        let ascending = times.windows(2).all(|t| t[0] < t[1]);
        let fits = times.len() == values.len() && times.iter().all(|t| t.is_finite());
        (ascending && fits).then_some(Motion { times, values })
    }

    /// Whether it holds no samples: the prim doesn't move.
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }
}

impl<T> Default for Motion<T> {
    fn default() -> Motion<T> {
        Motion::none()
    }
}

/// An Instancer's copies' placements: each the matrix's first three rows,
/// row-major.
pub type Copies = Arc<[[[f32; 4]; 3]]>;

/// A triangle mesh in its own space.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// Point positions.
    pub points: Arc<[[f32; 3]]>,
    /// Three point indices a triangle.
    pub triangles: Arc<[[u32; 3]]>,
    /// Shading normals, three a triangle (one a corner); none: flat.
    pub normals: Option<Arc<[[f32; 3]]>>,
    /// Texture coordinates, three a triangle; none: zero.
    pub uvs: Option<Arc<[[f32; 2]]>>,
    /// Each triangle's index into `materials` (an index past its end, or an
    /// empty list, takes the engine's default material).
    pub material_slots: Arc<[u16]>,
    /// The materials the slots name.
    pub materials: Arc<[MaterialId]>,
    /// The attributes its materials read by name (Attribute nodes); a host
    /// sends the ones some material names, not every attribute it has.
    pub attributes: Arc<[MeshAttribute]>,
    /// Object to world.
    pub transform: Matrix,
    /// Object to world across the shutter.
    pub motion: Motion<Matrix>,
    /// Whether it is rendered.
    pub visible: bool,
    /// The object id the ObjectId output writes.
    pub id: u32,
    /// The host's path of the object (its node path): the name
    /// Cryptomatte gives it.
    pub path: Arc<str>,
}

impl Mesh {
    /// What the [`MATERIAL_ID`](crate::aov::standard::MATERIAL_ID) output
    /// writes where `triangle` is hit: its material's prim slot plus one,
    /// the same on every mesh bound to that material whatever slot it
    /// takes there; 0 where the engine's default material shades it (an
    /// index past `materials`, or a triangle past the mesh).
    pub fn material_id(&self, triangle: usize) -> u32 {
        (self.material_slots.get(triangle))
            .and_then(|&slot| self.materials.get(usize::from(slot)))
            .map_or(0, |m| m.slot + 1)
    }
}

/// Where a mesh attribute's values sit, and how a hit reads them (an
/// integer one never blends: [`Numbers::Integers`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttributeDomain {
    /// One value a point, interpolated across each triangle by its
    /// barycentric coordinates.
    Point,
    /// Three values a triangle (one a corner, in the triangle's corner
    /// order), interpolated across it: a polygon vertex attribute.
    Corner,
    /// One value a triangle, constant across it: a polygon's.
    Triangle,
    /// One value for the whole mesh.
    Constant,
}

/// A mesh attribute's values, `components` a value, interleaved.
#[derive(Clone, Debug, PartialEq)]
pub enum Numbers {
    /// Real numbers, blended across a triangle.
    Floats(Arc<[f32]>),
    /// Whole numbers (ids), exact: point and corner values take the
    /// nearest corner's (the largest barycentric weight) rather than a
    /// blend, so an id stays an id. An engine whose storage holds fewer
    /// bits refuses the attribute (it reports it) rather than rounding.
    Integers(Arc<[i64]>),
}

impl Numbers {
    /// How many numbers it holds.
    pub fn len(&self) -> usize {
        match self {
            Numbers::Floats(v) => v.len(),
            Numbers::Integers(v) => v.len(),
        }
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A named attribute of a mesh: `components` numbers a value (1 to 4).
#[derive(Clone, Debug, PartialEq)]
pub struct MeshAttribute {
    /// Its name, as Attribute nodes name it (`Cd`, `roughness`).
    pub name: Arc<str>,
    /// Where its values sit.
    pub domain: AttributeDomain,
    /// Numbers a value, 1 to 4.
    pub components: u32,
    /// Whether it holds colors (linear ACEScg, the host converted them
    /// from the working space; an Attribute node's float
    /// output is then the luminance). Colors are floats.
    pub color: bool,
    /// The values.
    pub values: Numbers,
}

impl MeshAttribute {
    /// Values the attribute must hold on a mesh of `points` points and
    /// `triangles` triangles.
    pub fn count(&self, points: usize, triangles: usize) -> usize {
        match self.domain {
            AttributeDomain::Point => points,
            AttributeDomain::Corner => triangles * 3,
            AttributeDomain::Triangle => triangles,
            AttributeDomain::Constant => 1,
        }
    }

    /// Whether it holds exactly one value per element of its domain, with
    /// 1 to 4 components and a name.
    pub fn is_consistent(&self, points: usize, triangles: usize) -> bool {
        !self.name.is_empty()
            && (1..=4).contains(&self.components)
            && Some(self.values.len())
                == self
                    .count(points, triangles)
                    .checked_mul(self.components as usize)
    }
}

/// Copies of a Mesh prim (prototype) at many placements.
#[derive(Clone, Debug, PartialEq)]
pub struct Instancer {
    /// The Mesh prim copied.
    pub prototype: PrimId,
    /// Each copy's placement.
    pub transforms: Copies,
    /// Each copy's placement across the shutter: all the copies a time.
    pub motion: Motion<Copies>,
    /// Whether the copies are rendered.
    pub visible: bool,
}

/// How a camera projects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Projection {
    /// Rays from the camera's position.
    #[default]
    Perspective,
    /// Parallel rays across the film back.
    Orthographic,
}

/// A camera after UsdGeomCamera. It looks down its local -Z with +Y up, and
/// its film back fits the image horizontally.
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    /// The projection.
    pub projection: Projection,
    /// The lens's focal length, in mm.
    pub focal_length: f32,
    /// The film back's width, in mm; orthographic: the view's width, a
    /// length in mm on the scene's scale.
    pub horizontal_aperture: f32,
    /// The film back's height, in mm (orthographic: the view's height).
    pub vertical_aperture: f32,
    /// The film back's horizontal shift, in mm.
    pub horizontal_aperture_offset: f32,
    /// The film back's vertical shift, in mm.
    pub vertical_aperture_offset: f32,
    /// Near and far clipping distances, in scene units.
    pub clipping_range: [f32; 2],
    /// The lens's f-number; 0 is a pinhole (no depth of field).
    pub f_stop: f32,
    /// The distance in focus, in scene units.
    pub focus_distance: f32,
    /// Millimetres in one scene unit (1000 for metres): what turns the lens
    /// and an orthographic film back into scene units.
    pub mm_per_unit: f32,
    /// When the shutter opens and closes, in frames relative to the frame
    /// rendered (`shutter:open`, `shutter:close`); equal: no motion blur.
    pub shutter: [f32; 2],
    /// Camera to world.
    pub transform: Matrix,
    /// Camera to world across the shutter.
    pub motion: Motion<Matrix>,
}

impl Camera {
    /// Whether its shutter stays open a while: what blurs motion.
    pub fn blurs(&self) -> bool {
        self.shutter[1] > self.shutter[0]
    }
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            projection: Projection::Perspective,
            focal_length: 50.0,
            horizontal_aperture: 36.0,
            vertical_aperture: 24.0,
            horizontal_aperture_offset: 0.0,
            vertical_aperture_offset: 0.0,
            clipping_range: [0.1, 10_000.0],
            f_stop: 0.0,
            focus_distance: 5.0,
            mm_per_unit: 1000.0,
            shutter: [0.0, 0.0],
            transform: IDENTITY,
            motion: Motion::none(),
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
}

/// What a texel's channels are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexelFormat {
    /// 8-bit unsigned normalized.
    U8,
    /// IEEE half floats (their bits, little-endian).
    F16,
    /// IEEE single floats (little-endian).
    F32,
}

impl TexelFormat {
    /// Bytes a channel takes.
    pub fn bytes(self) -> usize {
        match self {
            TexelFormat::U8 => 1,
            TexelFormat::F16 => 2,
            TexelFormat::F32 => 4,
        }
    }
}

/// How a texture's values map to light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureColorSpace {
    /// sRGB-encoded Rec.709 color (8-bit; engines decode it and convert it
    /// to ACEScg).
    Srgb,
    /// Linear ACEScg, the render's color space (hosts convert other spaces
    /// before sending).
    Linear,
    /// Data, read as stored.
    Raw,
}

/// Decoded pixels (the host decodes files; engines need no decoders).
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    /// Texels across.
    pub width: u32,
    /// Texels down; rows from the top.
    pub height: u32,
    /// Channels a texel (1 to 4).
    pub channels: u32,
    /// The channels' format.
    pub format: TexelFormat,
    /// `width × height × channels` values, interleaved.
    pub pixels: Arc<[u8]>,
    /// What the values mean.
    pub color_space: TextureColorSpace,
}

impl Texture {
    /// Whether the pixel array holds exactly `width × height × channels` values.
    pub fn is_consistent(&self) -> bool {
        (1..=4).contains(&self.channels)
            && (self.width as usize)
                .checked_mul(self.height as usize)
                .and_then(|n| n.checked_mul(self.channels as usize))
                .and_then(|n| n.checked_mul(self.format.bytes()))
                == Some(self.pixels.len())
    }
}

/// A prim's payload.
#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    /// A triangle mesh.
    Mesh(Mesh),
    /// Copies of a mesh.
    Instancer(Instancer),
    /// A camera.
    Camera(Camera),
    /// A prim of a type the engine declares (a light, fog).
    Declared(Declared),
    /// A material.
    Material(Material),
    /// A texture.
    Texture(Texture),
}

/// The kinds of prim: the structural ones, for what an engine supports
/// ([`crate::EngineDescription::prims`]), and declared prims, which an
/// engine takes of the types it declares.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimKind {
    Mesh,
    Instancer,
    Camera,
    Material,
    Texture,
    Declared,
}

impl PrimKind {
    /// The structural kinds.
    pub(crate) const STRUCTURAL: [PrimKind; 5] = [
        PrimKind::Mesh,
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
            Prim::Mesh(_) => PrimKind::Mesh,
            Prim::Instancer(_) => PrimKind::Instancer,
            Prim::Camera(_) => PrimKind::Camera,
            Prim::Declared(_) => PrimKind::Declared,
            Prim::Material(_) => PrimKind::Material,
            Prim::Texture(_) => PrimKind::Texture,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::PrimId;

    fn mesh(materials: &[PrimId], slots: &[u16]) -> Mesh {
        Mesh {
            points: Arc::from([[0.0; 3]; 3]),
            triangles: slots.iter().map(|_| [0, 1, 2]).collect(),
            normals: None,
            uvs: None,
            material_slots: Arc::from(slots),
            materials: Arc::from(materials),
            attributes: Arc::from([]),
            transform: IDENTITY,
            motion: Motion::none(),
            visible: true,
            id: 1,
            path: Arc::from("/m"),
        }
    }

    #[test]
    fn a_material_has_one_id_across_meshes() {
        let id = |slot| PrimId {
            slot,
            generation: 0,
        };
        let (red, blue) = (id(4), id(9));
        // Red is the first slot of one mesh and the second of another.
        let one = mesh(&[red, blue], &[0, 1]);
        let other = mesh(&[blue, red], &[1, 0, 7]);
        assert_eq!(one.material_id(0), other.material_id(0));
        assert_eq!(one.material_id(1), other.material_id(1));
        assert_ne!(one.material_id(0), one.material_id(1));
        assert_eq!(other.material_id(2), 0, "a slot past the list: the default");
        assert_eq!(one.material_id(5), 0, "a triangle past the mesh");
    }
}
