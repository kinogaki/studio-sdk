//! The standard form: a material as an OpenPBR Surface shown through a
//! Surface Material, what engines, the shaded viewport and file formats meet
//! in. A MaterialX material's is its surface translated
//! ([`Engines::standard`](crate::Engines::standard)); an engine's own
//! material's is what its rules say ([`MaterialRules::standard`](crate::MaterialRules::standard)),
//! made with [`open_pbr`].
//!
//! [`display`] reads the viewport's approximation from it: the base color, a
//! constant or an image's (its coordinates' transform, wrap and filter, as the
//! graphs MaterialX defines its texture nodes by place them) or a
//! geometric property's; an emissive surface with no base, its emission;
//! else grey.
use super::adopt;
use super::mtlx;
use super::{DisplayTexture, MaterialDisplay, MaterialNetwork, NodeIndex};
use kinogaki_geometry::Error;
use kinogaki_geometry::image::{SampleFilter, Sampling, Wrap};
use kinogaki_value::{Parameters, Value};
use std::sync::Arc;

/// OpenPBR Surface's node: the standard form's surface.
pub const OPEN_PBR: &str = "open_pbr_surface";

/// The shading models a host offers a new material as, by signature, in
/// order: OpenPBR Surface first, then USD Preview Surface, Autodesk's
/// Standard Surface, glTF PBR and Surface Unlit.
pub const MODELS: [&str; 5] = [
    "ND_open_pbr_surface_surfaceshader",
    "ND_UsdPreviewSurface_surfaceshader",
    "ND_standard_surface_surfaceshader",
    "ND_gltf_pbr_surfaceshader",
    "ND_surface_unlit",
];

/// The signature of an OpenPBR Surface.
const OPEN_PBR_SURFACE: &str = "ND_open_pbr_surface_surfaceshader";
/// The signature of a Surface Material.
const SURFACE_MATERIAL: &str = "ND_surfacematerial";

/// A material of one OpenPBR Surface holding `values` (by OpenPBR's names,
/// the rest at its defaults), shown through a Surface Material.
///
/// # Errors
/// A value OpenPBR Surface has no input for, or holds otherwise.
pub fn open_pbr(values: &Parameters) -> Result<MaterialNetwork, Error> {
    let surface = mtlx::def(OPEN_PBR_SURFACE).expect("MaterialX defines OpenPBR Surface");
    let material = mtlx::def(SURFACE_MATERIAL).expect("MaterialX defines Surface Material");
    let mut built = MaterialNetwork::builder();
    let s = built.node(&surface, values)?;
    let m = built.node(&material, &Parameters::new())?;
    built.link(s, "out", m, "surfaceshader")?;
    built.build(m)
}

/// What the shaded viewport shows of `network`, a standard form (see the
/// module docs).
pub fn display(network: &MaterialNetwork) -> MaterialDisplay {
    let grey = MaterialDisplay::color([0.5; 3]);
    let network = adopt::expanded(network, &[OPEN_PBR, "surfacematerial"]);
    let Some(surface) = network.surface() else {
        return grey;
    };
    let node = &network.nodes()[surface as usize];
    if node.def.mtlx().is_none_or(|d| d.node != OPEN_PBR) {
        return grey;
    }
    let number = |name: &str| node.values.get(name).and_then(Value::number).unwrap_or(0.0);
    let emissive = number("base_weight") == 0.0 && number("emission_luminance") > 0.0;
    let input = if emissive {
        "emission_color"
    } else {
        "base_color"
    };
    let held = node
        .values
        .get(input)
        .and_then(Value::color)
        .unwrap_or([0.5; 3]);
    let Some(link) = network
        .links()
        .iter()
        .find(|l| l.to == surface && &*l.input == input)
    else {
        return MaterialDisplay::color(held.map(|v| v as f32));
    };
    traced(&network, link.from, 0).unwrap_or(grey)
}

/// What node `at`'s color shows as: an image's, a geometric property's, or
/// both, tinted by the constants multiplying them; through any other node, by
/// its first wired input.
fn traced(network: &MaterialNetwork, at: NodeIndex, depth: usize) -> Option<MaterialDisplay> {
    let node = &network.nodes()[at as usize];
    let def = node.def.mtlx()?;
    let into = |input: &str| {
        network
            .links()
            .iter()
            .find(|l| l.to == at && &*l.input == input)
    };
    let white = || MaterialDisplay::color([1.0; 3]);
    if depth > 64 {
        return None;
    }
    match def.node.as_str() {
        "image" => {
            let image = node.images.get("file")?;
            let (transform, uv) = match into("texcoord") {
                Some(l) => affine(network, l.from)?,
                None => (IDENTITY, Arc::from(kinogaki_geometry::names::UV)),
            };
            let wrap = |name: &str| match node.values.get(name).and_then(Value::choice) {
                Some("clamp") => Wrap::Clamp,
                Some("mirror") => Wrap::MirrorRepeat,
                // MaterialX reads the node's default outside; the viewport black.
                Some("constant") => Wrap::Black,
                _ => Wrap::Repeat,
            };
            let filter = match node.values.get("filtertype").and_then(Value::choice) {
                Some("closest") => SampleFilter::Nearest,
                Some("cubic") => SampleFilter::Bicubic,
                _ => SampleFilter::Bilinear,
            };
            let mut texture = DisplayTexture::new(image.clone());
            texture.uv = uv;
            texture.transform = transform.map(|row| row.map(|v| v as f32));
            texture.sampling = Sampling {
                wrap: [wrap("uaddressmode"), wrap("vaddressmode")],
                filter,
            };
            Some(MaterialDisplay {
                texture: Some(texture),
                ..white()
            })
        }
        "geomcolor" => Some(MaterialDisplay {
            primvar: Some(Arc::from(kinogaki_geometry::names::CD)),
            ..white()
        }),
        "geompropvalue" => Some(MaterialDisplay {
            primvar: Some(Arc::from(node.values.get("geomprop")?.text()?)),
            ..white()
        }),
        "multiply" => {
            let mut shown = white();
            let mut any = false;
            for input in ["in1", "in2"] {
                match into(input) {
                    Some(l) => {
                        let part = traced(network, l.from, depth + 1)?;
                        shown.color = [0, 1, 2].map(|k| shown.color[k] * part.color[k]);
                        shown.texture = shown.texture.or(part.texture);
                        shown.primvar = shown.primvar.or(part.primvar);
                        any = true;
                    }
                    None => {
                        let factor = match node.values.get(input)? {
                            Value::Color(c) => c.map(|v| v as f32),
                            Value::Number(n) => [*n as f32; 3],
                            Value::Numbers(n) if n.len() >= 3 => {
                                [n[0], n[1], n[2]].map(|v| v as f32)
                            }
                            _ => return None,
                        };
                        shown.color = [0, 1, 2].map(|k| shown.color[k] * factor[k]);
                    }
                }
            }
            any.then_some(shown)
        }
        _ => {
            let first = def.inputs.iter().find_map(|p| into(&p.name))?;
            traced(network, first.from, depth + 1)
        }
    }
}

/// An affine map of texture coordinates, by rows: `(u, v, 1)` to `(u', v')`.
type Affine = [[f64; 3]; 2];

const IDENTITY: Affine = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

/// Coordinates as a primvar's moved by an affine map: the map, the primvar.
type Placed = (Affine, Arc<str>);

/// `b` after `a`.
fn then(a: Affine, b: Affine) -> Affine {
    let row = |r: [f64; 3]| {
        [
            r[0] * a[0][0] + r[1] * a[1][0],
            r[0] * a[0][1] + r[1] * a[1][1],
            r[0] * a[0][2] + r[1] * a[1][2] + r[2],
        ]
    };
    [row(b[0]), row(b[1])]
}

/// Node `at`'s vector2 output as an affine map of a primvar's coordinates,
/// and that primvar, when it is one: a texture coordinate set, moved by
/// constants (the arithmetic, rotations and constant choices MaterialX's
/// placement graphs are made of).
fn affine(network: &MaterialNetwork, at: NodeIndex) -> Option<Placed> {
    let node = &network.nodes()[at as usize];
    let def = node.def.mtlx()?;
    let into = |input: &str| {
        network
            .links()
            .iter()
            .find(|l| l.to == at && &*l.input == input)
    };
    let pair = |name: &str| -> Option<[f64; 2]> {
        match node.values.get(name)? {
            Value::Numbers(n) if n.len() == 2 => Some([n[0], n[1]]),
            Value::Number(n) => Some([*n; 2]),
            _ => None,
        }
    };
    let scalar = |name: &str| node.values.get(name).and_then(Value::number);
    let operand = |linked: &str, constant: &str| -> Option<(Placed, [f64; 2])> {
        Some((affine(network, into(linked)?.from)?, pair(constant)?))
    };
    match def.node.as_str() {
        "texcoord" => {
            let index = node
                .values
                .get("index")
                .and_then(Value::integer)
                .unwrap_or(0);
            let set = match index {
                0 => kinogaki_geometry::names::UV.to_owned(),
                n => format!("{}{}", kinogaki_geometry::names::UV, n + 1),
            };
            Some((IDENTITY, Arc::from(set)))
        }
        "geompropvalue" => Some((IDENTITY, Arc::from(node.values.get("geomprop")?.text()?))),
        "dot" => affine(network, into("in")?.from),
        "add" | "subtract" | "multiply" | "divide" => {
            let ((inner, set), c) = operand("in1", "in2")?;
            let step = match def.node.as_str() {
                "add" => [[1.0, 0.0, c[0]], [0.0, 1.0, c[1]]],
                "subtract" => [[1.0, 0.0, -c[0]], [0.0, 1.0, -c[1]]],
                "multiply" => [[c[0], 0.0, 0.0], [0.0, c[1], 0.0]],
                _ => [[1.0 / c[0], 0.0, 0.0], [0.0, 1.0 / c[1], 0.0]],
            };
            Some((then(inner, step), set))
        }
        "rotate2d" => {
            let (inner, set) = affine(network, into("in")?.from)?;
            let (sa, ca) = scalar("amount")?.to_radians().sin_cos();
            Some((then(inner, [[ca, sa, 0.0], [-sa, ca, 0.0]]), set))
        }
        "ifgreater" | "ifgreatereq" | "ifequal" => {
            let compared = |name: &str| {
                let v = node.values.get(name)?;
                v.number().or_else(|| v.integer().map(|i| i as f64))
            };
            let (a, b) = (compared("value1")?, compared("value2")?);
            let holds = match def.node.as_str() {
                "ifgreater" => a > b,
                "ifgreatereq" => a >= b,
                _ => a == b,
            };
            affine(network, into(if holds { "in1" } else { "in2" })?.from)
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "standard_tests.rs"]
mod tests;
