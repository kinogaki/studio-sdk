//! The scenes the conformance renders: one for each prim kind and standard
//! type the engine declares, each a final frame beside the same scene without
//! its subject (`check`).
//!
//! The subjects stand on a quad facing +Z, lit by the standard dome when
//! the engine declares it, seen by an orthographic camera from above a
//! corner; a light is placed above the quad, emitting down -Z. An engine
//! taking materials also renders MaterialX materials as a host sends them,
//! adopted ([`Engines::adopt`]): an OpenPBR Surface; a glTF PBR of an
//! occlusion-roughness-metallic texture, cut out by its alpha; a USD Preview
//! Surface whose color and opacity a texture gives, cut out at a threshold;
//! a Surface Unlit.
use super::{Make, beauty, id};
use kinogaki_render_api::geometry::image::{
    Encoding, Format, Image, ImageSource, Rect, Sample, TextureImage,
};
use kinogaki_render_api::geometry::{
    Attribute, Domain, Kind, Meaning, Primitives, Sampled, Vec3, Xform, names,
};
use kinogaki_render_api::material::mtlx;
use kinogaki_render_api::value::{Parameters, Value};
use kinogaki_render_api::{
    Camera, Declared, EngineDescription, Engines, Id, Instancer, Material, MaterialNetwork, Prim,
    PrimKind, Projection, Shape, Subsets, Topology, standard,
};
use std::sync::Arc;

/// One scene: what it shows, and what it stands in.
pub(super) struct Scene {
    pub name: String,
    subject: Vec<Prim>,
    setting: Vec<Prim>,
}

/// `scene` rendered with and without its subject: they must differ.
pub(super) fn check(make: Make<'_>, scene: &Scene) -> Result<(), String> {
    let with = [&scene.setting[..], &scene.subject[..]].concat();
    let shown = beauty(make, with)?;
    let without = beauty(make, scene.setting.clone())?;
    let differs = |(a, b): (&f32, &f32)| (a - b).abs() > 1e-3 * b.abs().max(1.0);
    let covered = (shown.chunks(4).zip(without.chunks(4)))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(differs))
        .count();
    match covered {
        0 => Err("it renders but shows nowhere".into()),
        _ => Ok(()),
    }
}

/// The standard dome, which the camera doesn't see, when the engine
/// declares it: what lights the subjects.
pub(super) fn dome(description: &EngineDescription) -> Option<Prim> {
    description.prim_type(standard::DOME_LIGHT)?;
    let light = standard::Light {
        shape: standard::LightShape::Dome {
            image: None,
            format: standard::DomeFormat::Automatic,
            pole: standard::Pole::Scene,
            ground: None,
        },
        ..standard::Light::default()
    };
    Some(Prim::Declared(filled(light.declared(), description)))
}

/// The scenes `description` declares the prims of.
pub(super) fn scenes(description: &EngineDescription) -> Vec<Scene> {
    let renders = |kind| description.capabilities().renders(kind);
    let dome = dome(description);
    let lit = |subject: Vec<Prim>| (dome.iter().cloned().collect(), subject);
    let mut scenes = Vec::new();
    let mut add = |name: &str, (setting, subject): (Vec<Prim>, Vec<Prim>)| {
        scenes.push(Scene {
            name: name.into(),
            subject,
            setting,
        });
    };
    if !renders(PrimKind::Triangles) {
        return scenes;
    }
    add("triangles", lit(vec![Prim::Shape(quad(1.0))]));
    add("triangles with primvars", lit(vec![Prim::Shape(dressed())]));
    if renders(PrimKind::Curves) {
        add("curves", lit(vec![Prim::Shape(curve())]));
    }
    if renders(PrimKind::Instancer) {
        // The prototype is prim 1 of the subject, after the setting's.
        let first = dome.iter().count();
        add("instances", lit(instances(first, 1)));
        add("nested instances", lit(instances(first, 2)));
    }
    if renders(PrimKind::Material) {
        let material = Material {
            network: Arc::default(),
            path: "/default".into(),
            double_sided: true,
        };
        let first = dome.iter().count();
        let slots = Subsets::new(vec![0u16; 2].into(), vec![Id::of(id(first))]);
        let shape = quad(1.0)
            .rebuilt()
            .subsets(slots)
            .build()
            .expect("one slot");
        add(
            "a default material",
            lit(vec![Prim::Material(material), Prim::Shape(shape)]),
        );
        for (name, which) in [
            ("an OpenPBR Surface", Model::OpenPbr),
            ("a glTF PBR of an ORM texture, cut out", Model::Gltf),
            (
                "a USD Preview Surface of a texture, cut out",
                Model::Preview,
            ),
            ("a Surface Unlit", Model::Unlit),
        ] {
            add(name, lit(materialx(description, first, which)));
        }
    }
    let floor = Prim::Shape(quad(1.0));
    for ty in standard::lights() {
        if description.prim_type(ty.id).is_none() || ty.id == standard::DOME_LIGHT {
            continue;
        }
        let mut light = filled(Declared::new(ty.id, ty.schema().values()), description);
        let above = Vec3::new(0.0, 0.0, 3.0);
        light.xform = Sampled::still(Xform::from_axes(Vec3::X, Vec3::Y, Vec3::Z, above));
        add(
            &ty.label,
            (vec![floor.clone()], vec![Prim::Declared(light)]),
        );
    }
    if let Some(dome) = dome {
        add("dome light", (vec![floor.clone()], vec![dome]));
    }
    scenes
}

/// The MaterialX materials a scene shows.
#[derive(Clone, Copy)]
enum Model {
    OpenPbr,
    Gltf,
    Preview,
    Unlit,
}

/// A node of MaterialX signature `signature` holding `values`.
pub(super) fn node(
    built: &mut kinogaki_render_api::NetworkBuilder,
    signature: &str,
    values: &[(&str, Value)],
) -> u32 {
    let def = mtlx::def(signature).expect("MaterialX defines it");
    let values: Parameters = values
        .iter()
        .map(|(n, v)| ((*n).to_owned(), v.clone()))
        .collect();
    built.node(&def, &values).expect("its values")
}

/// A MaterialX material on the dressed quad, the subject's prims from prim
/// `first` on: an orange OpenPBR Surface; an orange glTF PBR whose metallic
/// and roughness come from a 2 × 2 texture, its alpha 0.8 cut out at 0.5; a
/// USD Preview Surface whose diffuse color and opacity are a 2 × 2
/// texture's, half of it cut out at 0.5; an orange Surface Unlit. Adopted
/// for the engine as a host adopts it, its file read as the texture.
fn materialx(description: &EngineDescription, first: usize, model: Model) -> Vec<Prim> {
    let orange = Value::Color([0.9, 0.4, 0.1]);
    let mut built = MaterialNetwork::builder();
    let surface = match model {
        Model::OpenPbr => node(
            &mut built,
            "ND_open_pbr_surface_surfaceshader",
            &[("base_color", orange)],
        ),
        Model::Unlit => node(
            &mut built,
            "ND_surface_unlit",
            &[("emission_color", orange)],
        ),
        Model::Gltf => {
            let gltf = node(
                &mut built,
                "ND_gltf_pbr_surfaceshader",
                &[
                    ("base_color", orange),
                    ("alpha", Value::Number(0.8)),
                    ("alpha_mode", Value::Integer(1)),
                ],
            );
            let orm = node(&mut built, "ND_gltf_image_vector3_vector3_1_0", &[]);
            built
                .image(orm, "file", ImageSource::path("orm.png"))
                .expect("an image input");
            for (index, input) in [(2, "metallic"), (1, "roughness")] {
                let extract = node(
                    &mut built,
                    "ND_extract_vector3",
                    &[("index", Value::Integer(index))],
                );
                built.link(orm, "out", extract, "in").expect("a vector");
                built.link(extract, "out", gltf, input).expect("a number");
            }
            gltf
        }
        Model::Preview => {
            let preview = node(
                &mut built,
                "ND_UsdPreviewSurface_surfaceshader",
                &[("opacityThreshold", Value::Number(0.5))],
            );
            let leaf = node(&mut built, "ND_UsdUVTexture_23", &[]);
            built
                .image(leaf, "file", ImageSource::path("leaf.png"))
                .expect("an image input");
            let wires = [("rgb", "diffuseColor"), ("a", "opacity")];
            for (output, input) in wires {
                built.link(leaf, output, preview, input).expect("a wire");
            }
            preview
        }
    };
    let material = node(&mut built, "ND_surfacematerial", &[]);
    built
        .link(surface, "out", material, "surfaceshader")
        .expect("a surface");
    let network = built.build(material).expect("a whole material");
    let mut network = Engines::new(vec![description.clone()]).adopt(&network, description.id());
    let mut prims = Vec::new();
    let file = match model {
        Model::Gltf => Some("orm.png"),
        Model::Preview => Some("leaf.png"),
        _ => None,
    };
    if let Some(file) = file {
        let texture: Id<TextureImage> = Id::of(id(first));
        let window = Rect::new([0, 0], [2, 2]).expect("a window");
        let format = Format::rgba(Sample::F32, Encoding::Raw);
        // glTF: occlusion 1; rough, then smooth; metal, then not. USD: an
        // orange leaf, then a cut-out one.
        let (a, b) = match model {
            Model::Gltf => ([1.0f32, 0.9, 1.0, 1.0], [1.0f32, 0.1, 0.0, 1.0]),
            _ => ([0.9f32, 0.4, 0.1, 1.0], [0.9f32, 0.4, 0.1, 0.2]),
        };
        let texels: Vec<u8> = ([a, b, b, a].iter().flatten())
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        let image = Image::builder(window)
            .plane(format, &texels)
            .build()
            .expect("a texture");
        prims.push(Prim::Texture(TextureImage {
            image,
            decoding: None,
        }));
        let reading: Vec<(u32, String)> = (network.nodes().iter().zip(0..))
            .flat_map(|(node, at)| {
                (node.images().iter())
                    .filter(|(_, image)| image.name() == file)
                    .map(move |(input, _)| (at, input.clone()))
            })
            .collect();
        for (at, input) in reading {
            (network.set_texture(at, &input, texture)).expect("an image input");
        }
    }
    let material = Id::of(id(first + prims.len()));
    prims.push(Prim::Material(Material {
        network: Arc::new(network),
        path: "/materialx".into(),
        double_sided: true,
    }));
    let slots = Subsets::new(vec![0u16; 2].into(), vec![material]);
    let shape = dressed()
        .rebuilt()
        .subsets(slots)
        .build()
        .expect("one slot");
    prims.push(Prim::Shape(shape));
    prims
}

/// `prim` with every value `description`'s schema of its type names: the
/// prim's, or the default (as a host sends it).
fn filled(prim: Declared, description: &EngineDescription) -> Declared {
    let Some(ty) = description.prim_type(prim.type_id) else {
        return prim;
    };
    let values = (ty.schema().parameters())
        .map(|d| {
            let value = prim.value(&d.name).unwrap_or(&d.value);
            (d.name.to_string(), value.clone())
        })
        .collect();
    Declared { values, ..prim }
}

/// A square `side` across in the XY plane, facing +Z.
pub(super) fn quad(side: f32) -> Shape {
    let s = side;
    let points = vec![[-s, -s, 0.0], [s, -s, 0.0], [s, s, 0.0], [-s, s, 0.0]];
    let triangles = vec![[0, 1, 2], [0, 2, 3]];
    Shape::builder(points.into(), Topology::Triangles(triangles.into()))
        .path("/quad")
        .build()
        .expect("a whole quad")
}

/// The quad with shading normals, coordinates and a color at each corner.
fn dressed() -> Shape {
    let corners = [0usize, 1, 2, 0, 2, 3];
    let normal = Attribute::tuples(vec![[0.0f32, 0.0, 1.0]; 6], Meaning::Normal);
    let uv = [[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let uv = Attribute::tuples(corners.map(|c| uv[c]).to_vec(), Meaning::TextureCoord);
    let color = Attribute::tuples(vec![[0.9f32, 0.4, 0.1]; 6], Meaning::Color);
    (quad(1.0).rebuilt())
        .primvar(Domain::Vertex, names::N, normal.expect("unit normals"))
        .primvar(Domain::Vertex, names::UV, uv.expect("finite coordinates"))
        .primvar(Domain::Vertex, names::CD, color.expect("finite colors"))
        .build()
        .expect("a dressed quad")
}

/// A polyline across the quad's diagonal, a tenth of its side wide.
fn curve() -> Shape {
    let points = vec![[-0.8f32, -0.8, 0.2], [0.0, 0.0, 0.2], [0.8, 0.8, 0.2]];
    let line = Primitives::new(
        3,
        vec![0, 3].into(),
        vec![0, 1, 2].into(),
        vec![Kind::POLYLINE].into(),
        None,
    )
    .expect("one polyline");
    let width = Attribute::tuples(vec![0.2f32; 3], Meaning::None).expect("finite widths");
    Shape::builder(points.into(), Topology::Curves(Arc::new(line)))
        .primvar(Domain::Point, names::WIDTH, width)
        .path("/curve")
        .build()
        .expect("a whole curve")
}

/// A small quad placed three times (`depth` 1), or those three placed
/// twice by an Instancer of the Instancer (`depth` 2); the prototype is
/// prim `first`.
fn instances(first: usize, depth: usize) -> Vec<Prim> {
    let mut prims = vec![Prim::Shape(quad(0.25))];
    let row = |x: f64| Xform::from_axes(Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(x, 0.0, 0.0));
    for level in 0..depth {
        let placed = (level == 0).then(|| vec![row(-0.6), row(0.0), row(0.6)]);
        let xforms = placed.unwrap_or_else(|| {
            let up = |y: f64| Xform::from_axes(Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(0.0, y, 0.0));
            vec![up(-0.4), up(0.4)]
        });
        let instancer = Instancer::builder(vec![id(first + level)], xforms.into())
            .path(format!("/instances{level}"))
            .build()
            .expect("whole instances");
        prims.push(Prim::Instancer(instancer));
    }
    prims
}

/// An orthographic camera looking at the quad from above its corner.
pub(super) fn camera() -> Camera {
    let back = Vec3::new(1.0, 0.9, 1.3).normalized().expect("a direction");
    let right = Vec3::new(0.0, 1.0, 0.0)
        .cross(back)
        .normalized()
        .expect("a side");
    let eye = back * 4.0;
    // Film back in millimetres of a scene in metres.
    let across = 2.4 * 1.5 * 1000.0;
    Camera {
        projection: Projection::Orthographic,
        horizontal_aperture: across,
        vertical_aperture: across,
        clipping_range: [1e-3, 20.0],
        xform: Sampled::still(Xform::from_axes(right, back.cross(right), back, eye)),
        ..Camera::default()
    }
}
