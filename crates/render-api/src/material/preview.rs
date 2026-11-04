//! The material preview: a shape in the SDK, an image from the engine. A host
//! asks a material's engine for a [`PreviewImage`] of a [`PreviewRequest`]
//! ([`RenderEngine::preview`](crate::RenderEngine::preview)) and shows it when
//! it arrives; it never renders a material itself. The engine renders on its
//! own thread, by any means it likes; [`PreviewRequest::scene`] gives it the
//! request as prims it can render as any other scene.
use crate::id::{Id, IdAllocator, PrimId};
use crate::material::MaterialNetwork;
use crate::prim::{Camera, Material, Prim, Shape, Subsets, Texture, Topology};
use crate::standard;
use crate::update::SceneUpdate;
use kinogaki_geometry::{Attribute, Domain, Meaning, Sampled, Vec3, Xform, names};
use std::sync::Arc;

/// Rings and segments of the sphere.
const RINGS: u32 = 32;
const SEGMENTS: u32 = 64;
/// Half the floor's side.
const FLOOR: f32 = 40.0;

/// What a material is shown on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreviewShape {
    /// A unit sphere on a floor.
    #[default]
    Sphere,
}

/// What a material is lit by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreviewLighting {
    /// An even dome the camera sees, and a warm key light from the upper left.
    #[default]
    Studio,
}

/// A picture of a material, asked of its engine.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewRequest {
    /// The material, its colors in the render space and its files resolved.
    pub network: Arc<MaterialNetwork>,
    /// The Texture prims its nodes name.
    pub textures: Vec<(Id<Texture>, Texture)>,
    /// What it is shown on.
    pub shape: PreviewShape,
    /// What it is lit by.
    pub lighting: PreviewLighting,
    /// The image's side, in pixels.
    pub side: u32,
}

/// A preview's image: linear ACEScg RGBA, four floats a pixel, rows from the
/// top.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewImage {
    /// Its side, in pixels.
    pub side: u32,
    /// `side × side × 4` values.
    pub pixels: Arc<[f32]>,
}

/// A request as prims: what a scene of its own takes to show it.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewScene {
    /// Every prim, to apply to an empty scene.
    pub update: SceneUpdate,
    /// The camera it is seen through.
    pub camera: Id<Camera>,
}

impl PreviewRequest {
    /// The request as prims: the shape in the material (a floor in the
    /// engine's default), under the lighting, seen by a camera framing it.
    /// Their ids are their own, past those of its textures.
    pub fn scene(&self) -> PreviewScene {
        let taken = (self.textures.iter()).map(|(id, _)| id.prim().slot + 1);
        let mut ids = IdAllocator::past(taken.max().unwrap_or(0));
        let mut added: Vec<(PrimId, Prim)> = (self.textures.iter())
            .map(|(id, texture)| (id.prim(), Prim::Texture(texture.clone())))
            .collect();
        let material: Id<Material> = ids.allocate_as();
        let prim = Material {
            network: self.network.clone(),
            path: Arc::from("/preview/material"),
        };
        added.push((material.prim(), Prim::Material(prim)));
        added.push((ids.allocate(), Prim::Shape(sphere(material))));
        added.push((ids.allocate(), Prim::Shape(floor())));
        for light in lights(self.lighting) {
            added.push((ids.allocate(), Prim::Declared(light.declared())));
        }
        let camera: Id<Camera> = ids.allocate_as();
        let lens = Camera {
            focal_length: 50.0,
            horizontal_aperture: 24.0,
            vertical_aperture: 24.0,
            xform: placed([0.0, 1.1, 5.4], [0.0, -0.1, 0.0]),
            ..Camera::default()
        };
        added.push((camera.prim(), Prim::Camera(lens)));
        let update = SceneUpdate {
            added,
            ..SceneUpdate::default()
        };
        PreviewScene { update, camera }
    }
}

/// The lights of `lighting`.
fn lights(lighting: PreviewLighting) -> [standard::Light; 2] {
    let PreviewLighting::Studio = lighting;
    let dome = standard::Light {
        shape: standard::Shape::Dome {
            image: None,
            ground: None,
        },
        intensity: 0.6,
        // Seen behind the sphere: a light grey, not black.
        camera: true,
        ..standard::Light::default()
    };
    let key = standard::Light {
        shape: standard::Shape::Rect {
            width: 2.0,
            height: 2.0,
        },
        color: [1.0, 0.95, 0.9],
        intensity: 4.0,
        xform: placed([-3.0, 4.0, 3.0], [0.0; 3]),
        camera: false,
        ..standard::Light::default()
    };
    [dome, key]
}

/// Placed at `eye`, looking at `at` with +Y up: its -Z toward `at`.
fn placed(eye: [f64; 3], at: [f64; 3]) -> Sampled<Xform> {
    let unit = |v: Vec3| v * (1.0 / v.length());
    let eye = Vec3::from(eye);
    let back = unit(eye - Vec3::from(at));
    let right = unit(Vec3::new(0.0, 1.0, 0.0).cross(back));
    let up = back.cross(right);
    Sampled::still(Xform::from_axes(right, up, back, eye))
}

/// A unit sphere about the origin in `material`: smooth, its texture
/// coordinates around (u) and up (v), a seam column of its own so they don't
/// wrap.
fn sphere(material: Id<Material>) -> Shape {
    let mut points = Vec::new();
    let mut at = Vec::new();
    for ring in 0..=RINGS {
        let v = ring as f32 / RINGS as f32;
        let polar = std::f32::consts::PI * (1.0 - v);
        for segment in 0..=SEGMENTS {
            let u = segment as f32 / SEGMENTS as f32;
            let around = std::f32::consts::TAU * u;
            let (s, c) = polar.sin_cos();
            points.push([s * around.cos(), c, -s * around.sin()]);
            at.push([u, v]);
        }
    }
    let row = SEGMENTS + 1;
    let mut triangles = Vec::new();
    for ring in 0..RINGS {
        for segment in 0..SEGMENTS {
            let a = ring * row + segment;
            let (b, c, d) = (a + 1, a + row, a + row + 1);
            // The poles' rows meet in a point: one triangle there.
            if ring > 0 {
                triangles.push([a, b, d]);
            }
            if ring + 1 < RINGS {
                triangles.push([a, d, c]);
            }
        }
    }
    let corners = || triangles.iter().flatten().map(|&p| p as usize);
    let uvs: Vec<[f32; 2]> = corners().map(|p| at[p]).collect();
    // A unit sphere's normal is its point.
    let normals: Vec<[f32; 3]> = corners().map(|p| points[p]).collect();
    let uvs = Attribute::tuples(uvs, Meaning::TextureCoord).expect("finite coordinates");
    let normals = Attribute::tuples(normals, Meaning::Normal).expect("unit normals");
    let slots = vec![0u16; triangles.len()].into();
    Shape::builder(points.into(), Topology::Triangles(triangles.into()))
        .primvar(Domain::Vertex, names::UV, uvs)
        .primvar(Domain::Vertex, names::N, normals)
        .subsets(Subsets::new(slots, vec![material]))
        .path("/preview/sphere")
        .build()
        .expect("the preview's sphere is whole")
}

/// The floor: a square [`FLOOR`] each way from the sphere's foot, facing up.
fn floor() -> Shape {
    let s = FLOOR;
    let points = vec![[-s, -1.0, -s], [s, -1.0, -s], [s, -1.0, s], [-s, -1.0, s]];
    let triangles = vec![[0, 2, 1], [0, 3, 2]];
    Shape::builder(points.into(), Topology::Triangles(triangles.into()))
        .path("/preview/floor")
        .build()
        .expect("the preview's floor is whole")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prim::PrimKind;

    fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    #[test]
    fn the_sphere_is_closed_unit_and_faces_out() {
        let shape = sphere(Id::of(PrimId {
            slot: 0,
            generation: 0,
        }));
        let (points, Topology::Triangles(triangles)) = (shape.points(), shape.topology()) else {
            panic!("triangles");
        };
        assert!(
            points
                .iter()
                .all(|p| (p.iter().map(|c| c * c).sum::<f32>() - 1.0).abs() < 1e-5)
        );
        // Two triangles a quad but one at each pole.
        assert_eq!(triangles.len() as u32, 2 * SEGMENTS * (RINGS - 1));
        for t in triangles.iter() {
            let [a, b, c] = t.map(|p| points[p as usize]);
            let n = cross(sub(b, a), sub(c, a));
            let centre = [a[0] + b[0] + c[0], a[1] + b[1] + c[1], a[2] + b[2] + c[2]];
            let out: f32 = n.iter().zip(centre).map(|(n, c)| n * c).sum();
            assert!(out > 0.0, "{t:?} faces in");
        }
        assert!(shape.primvar("N").is_some() && shape.primvar("uv").is_some());
        let floor = floor();
        let (points, Topology::Triangles(triangles)) = (floor.points(), floor.topology()) else {
            panic!("triangles");
        };
        let [a, b, c] = triangles[0].map(|p| points[p as usize]);
        assert!(cross(sub(b, a), sub(c, a))[1] > 0.0, "the floor faces up");
    }

    #[test]
    fn a_request_is_a_scene_of_its_own() {
        let pixels = Arc::from(vec![0u8; 4]);
        let format = crate::prim::TexelFormat::U8;
        let space = crate::prim::TextureColorSpace::Raw;
        let texture = Texture::new([1, 1], 4, format, pixels, space).unwrap();
        let id: Id<Texture> = Id::of(PrimId {
            slot: 7,
            generation: 0,
        });
        let request = PreviewRequest {
            network: Arc::default(),
            textures: vec![(id, texture)],
            shape: PreviewShape::Sphere,
            lighting: PreviewLighting::Studio,
            side: 64,
        };
        let scene = request.scene();
        let count = |kind| {
            let kinds = scene.update.added.iter().map(|(_, p)| p.kind());
            kinds.filter(|k| *k == kind).count()
        };
        assert_eq!(count(PrimKind::Texture), 1);
        assert_eq!(count(PrimKind::Material), 1);
        assert_eq!(count(PrimKind::Triangles), 2);
        assert_eq!(count(PrimKind::Declared), 2);
        assert_eq!(count(PrimKind::Camera), 1);
        let mut slots: Vec<u32> = scene.update.added.iter().map(|(id, _)| id.slot).collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), scene.update.added.len(), "one id a prim");
        assert!(slots[0] == 7 && slots[1] == 8, "past its textures");
    }
}
