//! Scenes an engine renders as their twins: what MaterialX and the prims
//! define alike renders alike, whatever of it the engine renders.
//!
//! - A glTF base color cut out at 0.5 by its texture's alpha times its
//!   factor's and the vertex colors' (0.8 and 1.0 times 0.75 and 0.75: one
//!   cut by the vertex alpha alone, one kept), its texture's colors
//!   premultiplied as a host sends them, against a texture of data,
//!   straight, holding those products.
//! - A float image read into OpenPBR's opacity, against an image whose every
//!   channel holds the first: MaterialX's float image is its first channel.
//! - Instances colored by their own `Cd`, against shapes holding it, a
//!   material reading it.
//! - Instances whose material map renders one of their prototype's
//!   materials as another, against shapes holding the materials.
//! - An image placed by a `place2d` whose coordinates nothing drives (it
//!   reads its value, (0, 0), whatever the mesh's), against the texel there
//!   as a constant.
//!
//! An engine that renders neither twin's difference renders them alike; one
//! that renders them otherwise says what it doesn't render of the first.
use super::scenes::{dome, node, quad};
use super::{Make, alike, beauty, id, rendered};
use kinogaki_render_api::geometry::image::{Encoding, Format, Image, ImageSource, Rect, Sample};
use kinogaki_render_api::geometry::{
    Attribute, Attributes, Domain, Meaning, Sampled, Vec3, Xform, names,
};
use kinogaki_render_api::value::Value;
use kinogaki_render_api::{
    EngineDescription, Engines, Id, Instancer, Material, MaterialMaps, MaterialNetwork,
    NetworkBuilder, Prim, PrimKind, RENDER_SPACE, Shape, Subsets,
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
        checks.push(("a place2d's own coordinates", placed(make, description)));
    }
    if renders(PrimKind::Material) && renders(PrimKind::Instancer) {
        checks.push(("instances' own colors", instance_colors(make, description)));
        checks.push(("instances' material maps", material_maps(make, description)));
    }
    checks
}

/// Two scenes rendered alike, or the engine saying what it doesn't render
/// of the first.
fn twins(make: Make<'_>, a: Vec<Prim>, b: Vec<Prim>) -> Result<(), String> {
    let (first, said) = rendered(make, a)?;
    match alike(&first, &beauty(make, b)?) {
        Err(_) if !said.is_empty() => Ok(()),
        outcome => outcome,
    }
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
        let reading: Vec<(u32, String)> = (network.nodes().iter().zip(0..))
            .flat_map(|(node, at)| node.images().keys().map(move |input| (at, input.clone())))
            .collect();
        for (at, input) in reading {
            (network.set_texture(at, &input, Id::of(id(texture)))).expect("an image input");
        }
    }
    Prim::Material(Material {
        network: Arc::new(network),
        path: "/twin".into(),
        double_sided: true,
    })
}

/// The quad `side` across at `at`, with texture coordinates, rendering
/// with prim `material`, holding `color` (and `alpha`) at every corner
/// (none: no color).
fn painted(side: f32, at: Xform, material: usize, color: Option<([f32; 3], f32)>) -> Shape {
    let uv = [[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let corners: Vec<[f32; 2]> = [0usize, 1, 2, 0, 2, 3].map(|c| uv[c]).to_vec();
    let uv = Attribute::tuples(corners, Meaning::TextureCoord).expect("finite coordinates");
    let slots = Subsets::new(vec![0u16; 2].into(), vec![Id::of(id(material))]);
    let mut built = (quad(side).rebuilt())
        .primvar(Domain::Vertex, names::UV, uv)
        .subsets(slots)
        .xform(Sampled::still(at));
    if let Some((color, alpha)) = color {
        let colors = Attribute::tuples(vec![color; 6], Meaning::Color).expect("finite colors");
        let alphas = Attribute::tuples(vec![alpha; 6], Meaning::None).expect("finite alphas");
        built = (built.primvar(Domain::Vertex, names::CD, colors)).primvar(
            Domain::Vertex,
            names::ALPHA,
            alphas,
        );
    }
    built.build().expect("a painted quad")
}

/// The dome where the engine declares one, and the first prim after it.
fn lit(description: &EngineDescription) -> (Vec<Prim>, usize) {
    let lit: Vec<Prim> = dome(description).into_iter().collect();
    let first = lit.len();
    (lit, first)
}

/// glTF's base color cut out at 0.5: a texture of grey 0.8 and alphas 0.8
/// and 1.0 (its colors premultiplied, as a host sends them), read at the
/// nearest texel, times a factor of grey 0.8 and alpha 0.75 and the quad's
/// vertex colors (grey 0.5, alpha 0.75), against a texture of data holding
/// those products (grey 0.32, alphas 0.45 and 0.5625, straight) with
/// neither. Without the vertex alpha the first (0.6) would show.
fn cut_out(make: Make<'_>, description: &EngineDescription) -> Result<(), String> {
    let scene = |texels: ([f32; 4], [f32; 4]), factored: bool| {
        let (mut prims, first) = lit(description);
        let encoding = match factored {
            true => Encoding::color(RENDER_SPACE, true),
            false => Encoding::Raw,
        };
        prims.push(Prim::Texture(texture(texels.0, texels.1, encoding)));
        let mut built = MaterialNetwork::builder();
        let mask = [
            ("alpha_mode", Value::Integer(1)),
            ("alpha_cutoff", Value::Number(0.5)),
        ];
        let surface = node(&mut built, "ND_gltf_pbr_surfaceshader", &mask);
        // The nearest texel: a quarter of the quad each.
        let mut values = vec![("filtertype", Value::Choice("closest".into()))];
        if factored {
            values.push(("color", Value::Numbers(vec![0.8, 0.8, 0.8, 0.75])));
        }
        let image = node(&mut built, "ND_kinogaki_gltf_colorimage", &values);
        (built.image(image, "file", ImageSource::path("base.png"))).expect("an image input");
        if factored {
            let vertex = node(&mut built, "ND_geomcolor_color4", &[]);
            (built.link(vertex, "out", image, "geomcolor")).expect("a color");
        }
        (built.link(image, "outcolor", surface, "base_color")).expect("a color");
        (built.link(image, "outa", surface, "alpha")).expect("a number");
        prims.push(material(built, surface, description, Some(first)));
        let vertex = factored.then_some(([0.5; 3], 0.75));
        prims.push(Prim::Shape(painted(
            1.0,
            Xform::IDENTITY,
            first + 1,
            vertex,
        )));
        prims
    };
    let factored = scene(([0.8, 0.8, 0.8, 0.8], [0.8, 0.8, 0.8, 1.0]), true);
    let product = scene(
        ([0.32, 0.32, 0.32, 0.45], [0.32, 0.32, 0.32, 0.5625]),
        false,
    );
    twins(make, factored, product)
}

/// An OpenPBR Surface whose base color an image gives (its nearest texel),
/// placed by a `place2d` whose coordinates nothing drives, moved by (0.75,
/// 0.25): it reads (0.75, 0.25), a texel of grey 0.2, everywhere; against
/// that grey as the base color.
fn placed(make: Make<'_>, description: &EngineDescription) -> Result<(), String> {
    let (mut texture_scene, first) = lit(description);
    let grey = |g: f32| [g, g, g, 1.0];
    texture_scene.push(Prim::Texture(texture(grey(0.2), grey(0.7), Encoding::Raw)));
    let mut built = MaterialNetwork::builder();
    let surface = node(&mut built, "ND_open_pbr_surface_surfaceshader", &[]);
    let closest = [("filtertype", Value::Choice("closest".into()))];
    let image = node(&mut built, "ND_image_color3", &closest);
    (built.image(image, "file", ImageSource::path("base.png"))).expect("an image input");
    let moved = [("offset", Value::Numbers(vec![-0.75, -0.25]))];
    let place = node(&mut built, "ND_place2d_vector2", &moved);
    (built.link(place, "out", image, "texcoord")).expect("coordinates");
    (built.link(image, "out", surface, "base_color")).expect("a color");
    texture_scene.push(material(built, surface, description, Some(first)));
    texture_scene.push(Prim::Shape(painted(1.0, Xform::IDENTITY, first + 1, None)));
    let (mut constant, first) = lit(description);
    let mut built = MaterialNetwork::builder();
    let base = [("base_color", Value::Color([0.2; 3]))];
    let surface = node(&mut built, "ND_open_pbr_surface_surfaceshader", &base);
    constant.push(material(built, surface, description, None));
    constant.push(Prim::Shape(painted(1.0, Xform::IDENTITY, first, None)));
    twins(make, texture_scene, constant)
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
        shapes.push(Prim::Shape(painted(0.25, at, first, Some((color, 1.0)))));
    }
    twins(make, instanced, shapes)
}

/// Two quads in a row, each half red and half green (a material slot a
/// triangle), through OpenPBR Surfaces of those base colors: placed by an
/// Instancer whose second instance's map renders its red half blue, against
/// two shapes holding those materials.
fn material_maps(make: Make<'_>, description: &EngineDescription) -> Result<(), String> {
    let row = |x: f64| Xform::from_axes(Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(x, 0.0, 0.0));
    let places = [row(-0.3), row(0.3)];
    // Red, green and blue Materials, the first at the index it gives.
    let colored = |prims: &mut Vec<Prim>| {
        let first = prims.len();
        for color in [[0.8, 0.1, 0.1], [0.1, 0.8, 0.1], [0.1, 0.1, 0.8]] {
            let mut built = MaterialNetwork::builder();
            let base = [("base_color", Value::Color(color))];
            let surface = node(&mut built, "ND_open_pbr_surface_surfaceshader", &base);
            prims.push(material(built, surface, description, None));
        }
        first
    };
    let halves = |at: Xform, [a, b]: [usize; 2]| {
        let slots = Subsets::new(vec![0u16, 1].into(), vec![Id::of(id(a)), Id::of(id(b))]);
        (quad(0.25)
            .rebuilt()
            .subsets(slots)
            .xform(Sampled::still(at)))
        .build()
        .expect("a quad of two halves")
    };
    let (mut instanced, _) = lit(description);
    let m = colored(&mut instanced);
    instanced.push(Prim::Shape(halves(Xform::IDENTITY, [m, m + 1])));
    let map = Arc::from([(Id::of(id(m)), Id::of(id(m + 2)))]);
    let maps = MaterialMaps::new(vec![Subsets::DEFAULT, 0].into(), vec![map]);
    let instancer = Instancer::builder(vec![id(m + 3)], places.to_vec().into())
        .material_maps(maps)
        .path("/mapped")
        .build()
        .expect("whole instances");
    instanced.push(Prim::Instancer(instancer));
    let (mut shapes, _) = lit(description);
    let m = colored(&mut shapes);
    shapes.push(Prim::Shape(halves(places[0], [m, m + 1])));
    shapes.push(Prim::Shape(halves(places[1], [m + 2, m + 1])));
    twins(make, instanced, shapes)
}
