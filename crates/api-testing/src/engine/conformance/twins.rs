//! Scenes an engine renders as their twins: what MaterialX and the prims
//! define alike renders alike, whatever of it the engine renders.
//!
//! - A glTF base color cut out at 0.5 by its texture's alpha times its
//!   factor's and the vertex colors' (0.6 and 1.0 times 0.75: one cut, one
//!   kept), against a texture holding those products and no factor.
//! - A float image read into OpenPBR's opacity, against an image whose every
//!   channel holds the first: MaterialX's float image is its first channel.
//! - Instances colored by their own `Cd`, against shapes holding it, a
//!   material reading it.
//!
//! An engine that renders neither twin's difference renders them alike.
use super::scenes::{dome, node, quad};
use super::{Make, alike, beauty, id};
use kinogaki_render_api::geometry::image::{Encoding, Format, Image, ImageSource, Rect, Sample};
use kinogaki_render_api::geometry::{
    Attribute, Attributes, Domain, Meaning, Sampled, Vec3, Xform, names,
};
use kinogaki_render_api::value::Value;
use kinogaki_render_api::{
    EngineDescription, Engines, Id, Instancer, Material, MaterialNetwork, NetworkBuilder, Prim,
    PrimKind, RENDER_SPACE, Shape, Subsets,
};
use std::sync::Arc;

/// Each check's name and outcome, for what the engine renders.
pub(super) fn checks(
    make: Make<'_>,
    description: &EngineDescription,
) -> Vec<(&'static str, Result<(), String>)> {
    let renders = |kind| description.capabilities().renders(kind);
    let mut checks = Vec::new();
    if renders(PrimKind::Material) && renders(PrimKind::Texture) {
        checks.push((
            "a glTF cut-out through its factor and vertex colors",
            cut_out(make, description),
        ));
        checks.push((
            "a float image's first channel",
            first_channel(make, description),
        ));
    }
    if renders(PrimKind::Material) && renders(PrimKind::Instancer) {
        checks.push(("instances' own colors", instance_colors(make, description)));
    }
    checks
}

/// Two scenes rendered, alike.
fn twins(make: Make<'_>, a: Vec<Prim>, b: Vec<Prim>) -> Result<(), String> {
    alike(&beauty(make, a)?, &beauty(make, b)?)
}

/// A 2 × 2 texture of RGBA floats, `a` at two opposite corners and `b` at
/// the others: straight values, premultiplied where `encoding` says (as a
/// host sends colors).
fn texture(a: [f32; 4], b: [f32; 4], encoding: Encoding) -> Image {
    let window = Rect::new([0, 0], [2, 2]).expect("a window");
    let premultiplied = matches!(
        encoding,
        Encoding::Color {
            premultiplied: true,
            ..
        }
    );
    let stored = |[r, g, b, a]: [f32; 4]| match premultiplied {
        true => [r * a, g * a, b * a, a],
        false => [r, g, b, a],
    };
    let texels: Vec<u8> = ([a, b, b, a].map(stored).iter().flatten())
        .flat_map(|v| v.to_ne_bytes())
        .collect();
    (Image::builder(window).plane(Format::rgba(Sample::F32, encoding), &texels))
        .build()
        .expect("a texture")
}

/// `built`'s surface `surface` shown by a Surface Material, adopted for the
/// engine, its images reading prim `texture`: the Material prim.
fn material(
    mut built: NetworkBuilder,
    surface: u32,
    description: &EngineDescription,
    texture: Option<usize>,
) -> Prim {
    let shown = node(&mut built, "ND_surfacematerial", &[]);
    (built.link(surface, "out", shown, "surfaceshader")).expect("a surface");
    let network = built.build(shown).expect("a whole material");
    let mut network = Engines::new(vec![description.clone()]).adopt(&network, description.id());
    if let Some(texture) = texture {
        for node in network.nodes_mut() {
            let read: Vec<String> = node.images.keys().cloned().collect();
            for input in read {
                node.textures.insert(input, Id::of(id(texture)));
            }
        }
    }
    Prim::Material(Material {
        network: Arc::new(network),
        path: "/twin".into(),
        double_sided: true,
    })
}

/// The quad `side` across at `at`, with texture coordinates, rendering
/// with prim `material`, holding `color` at every corner (none: no color).
fn painted(side: f32, at: Xform, material: usize, color: Option<[f32; 3]>) -> Shape {
    let uv = [[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let corners: Vec<[f32; 2]> = [0usize, 1, 2, 0, 2, 3].map(|c| uv[c]).to_vec();
    let uv = Attribute::tuples(corners, Meaning::TextureCoord).expect("finite coordinates");
    let slots = Subsets::new(vec![0u16; 2].into(), vec![Id::of(id(material))]);
    let mut built = (quad(side).rebuilt())
        .primvar(Domain::Vertex, names::UV, uv)
        .subsets(slots)
        .xform(Sampled::still(at));
    if let Some(color) = color {
        let colors = Attribute::tuples(vec![color; 6], Meaning::Color).expect("finite colors");
        built = built.primvar(Domain::Vertex, names::CD, colors);
    }
    built.build().expect("a painted quad")
}

/// The dome where the engine declares one, and the first prim after it.
fn lit(description: &EngineDescription) -> (Vec<Prim>, usize) {
    let lit: Vec<Prim> = dome(description).into_iter().collect();
    let first = lit.len();
    (lit, first)
}

/// glTF's base color cut out at 0.5: a texture of grey 0.8 and alphas 0.6
/// and 1.0, read at the nearest texel, times a factor of grey 0.8 and alpha
/// 0.75 and the quad's vertex colors (grey 0.5), against a texture of those
/// products (grey 0.32, alphas 0.45 and 0.75) with neither.
fn cut_out(make: Make<'_>, description: &EngineDescription) -> Result<(), String> {
    let scene = |texels: ([f32; 4], [f32; 4]), factor: bool| {
        let (mut prims, first) = lit(description);
        let color = Encoding::color(RENDER_SPACE, true);
        prims.push(Prim::Texture(texture(texels.0, texels.1, color)));
        let mut built = MaterialNetwork::builder();
        let mask = [
            ("alpha_mode", Value::Integer(1)),
            ("alpha_cutoff", Value::Number(0.5)),
        ];
        let surface = node(&mut built, "ND_gltf_pbr_surfaceshader", &mask);
        // The nearest texel: a quarter of the quad each.
        let mut values = vec![("filtertype", Value::Choice("closest".into()))];
        if factor {
            values.push(("color", Value::Numbers(vec![0.8, 0.8, 0.8, 0.75])));
        }
        let image = node(&mut built, "ND_kinogaki_gltf_colorimage", &values);
        (built.image(image, "file", ImageSource::path("base.png"))).expect("an image input");
        if factor {
            let vertex = node(&mut built, "ND_geomcolor_color4", &[]);
            (built.link(vertex, "out", image, "geomcolor")).expect("a color");
        }
        (built.link(image, "outcolor", surface, "base_color")).expect("a color");
        (built.link(image, "outa", surface, "alpha")).expect("a number");
        prims.push(material(built, surface, description, Some(first)));
        let color = factor.then_some([0.5; 3]);
        prims.push(Prim::Shape(painted(1.0, Xform::IDENTITY, first + 1, color)));
        prims
    };
    let factored = scene(([0.8, 0.8, 0.8, 0.6], [0.8, 0.8, 0.8, 1.0]), true);
    let product = scene(([0.32, 0.32, 0.32, 0.45], [0.32, 0.32, 0.32, 0.75]), false);
    twins(make, factored, product)
}

/// An orange OpenPBR Surface whose opacity a float image gives: of
/// (0.7, 0.2, 0.1), against (0.7, 0.7, 0.7).
fn first_channel(make: Make<'_>, description: &EngineDescription) -> Result<(), String> {
    let scene = |texel: [f32; 4]| {
        let (mut prims, first) = lit(description);
        prims.push(Prim::Texture(texture(texel, texel, Encoding::Raw)));
        let mut built = MaterialNetwork::builder();
        let orange = [("base_color", Value::Color([0.9, 0.4, 0.1]))];
        let surface = node(&mut built, "ND_open_pbr_surface_surfaceshader", &orange);
        let image = node(&mut built, "ND_image_float", &[]);
        (built.image(image, "file", ImageSource::path("opacity.png"))).expect("an image input");
        (built.link(image, "out", surface, "geometry_opacity")).expect("a number");
        prims.push(material(built, surface, description, Some(first)));
        prims.push(Prim::Shape(painted(1.0, Xform::IDENTITY, first + 1, None)));
        prims
    };
    twins(
        make,
        scene([0.7, 0.2, 0.1, 1.0]),
        scene([0.7, 0.7, 0.7, 1.0]),
    )
}

/// Three small quads in a row, red, green and blue, through an OpenPBR
/// Surface whose base color is the geometric color: placed by an
/// Instancer with each instance's `Cd`, against three shapes holding them.
fn instance_colors(make: Make<'_>, description: &EngineDescription) -> Result<(), String> {
    let colors = [[0.8f32, 0.1, 0.1], [0.1, 0.8, 0.1], [0.1, 0.1, 0.8]];
    let row = |x: f64| Xform::from_axes(Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(x, 0.0, 0.0));
    let places = [row(-0.6), row(0.0), row(0.6)];
    let reading = || {
        let mut built = MaterialNetwork::builder();
        let surface = node(&mut built, "ND_open_pbr_surface_surfaceshader", &[]);
        let color = node(&mut built, "ND_geomcolor_color3", &[]);
        (built.link(color, "out", surface, "base_color")).expect("a color");
        material(built, surface, description, None)
    };
    let (mut instanced, first) = lit(description);
    instanced.push(reading());
    instanced.push(Prim::Shape(painted(0.25, Xform::IDENTITY, first, None)));
    let cd = Attribute::tuples(colors.to_vec(), Meaning::Color).expect("finite colors");
    let primvars = (Attributes::new().with(Domain::Instance, names::CD, cd)).expect("a color each");
    let instancer = Instancer::builder(vec![id(first + 1)], places.to_vec().into())
        .primvars(primvars)
        .path("/colored")
        .build()
        .expect("whole instances");
    instanced.push(Prim::Instancer(instancer));
    let (mut shapes, first) = lit(description);
    shapes.push(reading());
    for (at, color) in places.into_iter().zip(colors) {
        shapes.push(Prim::Shape(painted(0.25, at, first, Some(color))));
    }
    twins(make, instanced, shapes)
}
