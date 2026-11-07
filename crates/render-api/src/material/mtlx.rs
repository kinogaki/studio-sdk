//! MaterialX's nodes as [`NodeDef`]s: one a signature of the
//! [MaterialX library](kinogaki_mtlx::library), made on first use and shared
//! ([`def`]). A node's type is `kinogaki.mtlx.<node>` ([`id`]), its label
//! MaterialX's own or its name in words, its family its node group; its
//! inputs keep MaterialX's names, defaults, enumerations and UI hints:
//!
//! - a uniform input is a setting, any other value input one a wire may
//!   drive;
//! - an input reading a geometric property while unwired (a texture
//!   coordinate, the shading normal), a matrix and a closure are wires alone;
//! - a `filename` input is an [`Image`](Socket::Image) wire alone: the host's
//!   Image node reads the file, and holds what MaterialX's `colorspace`
//!   attribute says.
use super::def::{Input, NodeDef, Output, Socket};
use kinogaki_mtlx::{Port, Type, library};
use kinogaki_value::NodeTypeId;
use kinogaki_value::{Choice, Number, NumberLook, Parameter, Section, Value, ValueKind};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// MaterialX's node `node`'s type id (`kinogaki.mtlx.open_pbr_surface`).
pub fn id(node: &str) -> Option<NodeTypeId> {
    NodeTypeId::parse(&format!("kinogaki.mtlx.{node}"))
}

/// The node definition of MaterialX signature `signature`
/// (`ND_image_color3`), made once and shared.
pub fn def(signature: &str) -> Option<Arc<NodeDef>> {
    static MADE: Mutex<Option<HashMap<String, Arc<NodeDef>>>> = Mutex::new(None);
    let mut made = MADE.lock().unwrap_or_else(|e| e.into_inner());
    let made = made.get_or_insert_with(HashMap::new);
    if let Some(def) = made.get(signature) {
        return Some(def.clone());
    }
    let source = library().def(signature)?;
    let def = Arc::new(make(source.clone())?);
    made.insert(signature.to_owned(), def.clone());
    Some(def)
}

/// The parameter a MaterialX node holds its signature in, a definition's
/// name (`ND_image_color3`).
pub const SIGNATURE: &str = "signature";

/// `source` as a node definition.
fn make(source: Arc<kinogaki_mtlx::NodeDef>) -> Option<NodeDef> {
    let mut def = NodeDef::new(
        id(&source.node)?,
        &label_of(&source),
        &family(&source.group),
    );
    def.aliases = vec![Arc::from(source.node.as_str())];
    for port in &source.inputs {
        let folder = port.folder.as_deref().unwrap_or("");
        if !folder.is_empty() && !def.groups.iter().any(|g| g.name == folder) {
            let first = def.groups.is_empty();
            def.groups.push(Section::new(folder, folder, first));
        }
        def = def.input(input(&source.node, port));
    }
    for port in &source.outputs {
        def = def.output(Output::new(
            &port.name,
            &words(&port.name),
            Socket::of(&port.ty),
        ));
    }
    Some(def.of_mtlx(source))
}

/// `port` of node `node` as an input.
fn input(node: &str, port: &Port) -> Input {
    let socket = Socket::of(&port.ty);
    let label = port.label.clone().unwrap_or_else(|| words(&port.name));
    let wire = port.geomprop.is_some()
        || matches!(
            port.ty,
            Type::Matrix33 | Type::Matrix44 | Type::Filename | Type::Named(_)
        );
    let kind = kind(node, port);
    let (Some(kind), false) = (kind, wire) else {
        return Input::wire(&port.name, &label, socket);
    };
    let default = (port.value.as_ref())
        .and_then(|v| value(&kind, v))
        .unwrap_or_else(|| zero(&kind));
    let mut parameter = Parameter::new(&port.name, label, kind, default)
        .group(port.folder.as_deref().unwrap_or(""))
        .help(port.doc.as_str());
    if let ValueKind::Number(number) = &parameter.kind
        && number.soft.is_some()
    {
        parameter = parameter.look(NumberLook::slider());
    }
    match port.uniform {
        true => Input::Setting(parameter.animatable(false)),
        false => Input::Driven(parameter, socket),
    }
}

/// The kind a value of `port` of node `node` is held as; none for a type
/// holding none.
fn kind(node: &str, port: &Port) -> Option<ValueKind> {
    let span = |n: Number| -> Number {
        let low = (port.soft_min.as_ref().or(port.min.as_ref()))
            .and_then(|v| v.numbers()?.first().copied());
        let high = (port.soft_max.as_ref().or(port.max.as_ref()))
            .and_then(|v| v.numbers()?.first().copied());
        match (low, high) {
            (Some(a), Some(b)) if a < b => n.soft(a, b),
            _ => n,
        }
    };
    let choices = || {
        (port.enumeration.iter())
            .map(|key| {
                Choice::new(
                    key,
                    &if key.is_empty() {
                        "None".to_owned()
                    } else {
                        words(key)
                    },
                )
            })
            .collect()
    };
    Some(match port.ty {
        Type::Boolean => ValueKind::Toggle,
        Type::Integer if !port.enumeration.is_empty() => {
            let values: Vec<i64> = (port.enum_values.iter())
                .filter_map(|v| match v {
                    kinogaki_mtlx::Value::Integer(n) => Some(*n),
                    _ => None,
                })
                .collect();
            let first = values.first().copied().unwrap_or(0);
            let menu = port.enumeration.iter().map(|e| words(e)).collect();
            ValueKind::Integer {
                min: first,
                max: first + port.enumeration.len() as i64 - 1,
                menu,
            }
        }
        Type::Integer => ValueKind::integer(i64::from(i32::MIN), i64::from(i32::MAX)),
        Type::Float => {
            let number = match port.unit_type.as_deref() {
                Some("angle") => Number::angle(),
                _ if length(node, port) => Number::length(),
                _ => Number::plain(),
            };
            ValueKind::Number(span(number))
        }
        Type::Color3 => ValueKind::Color,
        Type::Color4 => ValueKind::Channels(Number::plain()),
        Type::Vector2 | Type::Vector3 | Type::Vector4 => ValueKind::Vector {
            components: port.ty.width() as u8,
            number: match length(node, port) {
                true => Number::length(),
                false => Number::plain(),
            },
        },
        Type::String if !port.enumeration.is_empty() => ValueKind::choice(choices()),
        Type::String => ValueKind::Text,
        _ => return None,
    })
}

/// Whether input `input` of MaterialX definition `def` is a length in the
/// scene's units (its kind a `Length`), which a format converts between
/// units as it converts geometry.
pub fn is_length(def: &kinogaki_mtlx::NodeDef, input: &str) -> bool {
    def.input(input).is_some_and(|port| length(&def.node, port))
}

/// Whether input `port` of node `node` is a length in the scene's units,
/// which a format converts as it converts geometry: a distance its
/// definition fixes no unit for, or one of [`LENGTHS`].
fn length(node: &str, port: &Port) -> bool {
    let distance = port.unit_type.as_deref() == Some("distance") && port.unit.is_none();
    distance || LENGTHS.contains(&(node, port.name.as_str()))
}

/// The shading models' inputs that are lengths in the scene's units, which
/// MaterialX's definitions don't say: glTF's volume (in metres in a glTF
/// file), and OpenPBR's and Standard Surface's depths and scattering
/// distances (Standard Surface's as the scale of its radius color).
const LENGTHS: &[(&str, &str)] = &[
    ("gltf_pbr", "thickness"),
    ("gltf_pbr", "attenuation_distance"),
    ("open_pbr_surface", "transmission_depth"),
    ("open_pbr_surface", "subsurface_radius"),
    ("standard_surface", "transmission_depth"),
    ("standard_surface", "subsurface_scale"),
];

/// MaterialX value `v` as a value of `kind`, the kind a node's input holds
/// it as, when it is one: what a format reads of a document's value.
pub fn value(kind: &ValueKind, v: &kinogaki_mtlx::Value) -> Option<Value> {
    use kinogaki_mtlx::Value as M;
    Some(match (kind, v) {
        (ValueKind::Toggle, M::Boolean(on)) => Value::Bool(*on),
        (ValueKind::Integer { .. }, M::Integer(n)) => Value::Integer(*n),
        (ValueKind::Number(_), M::Numbers(n)) if n.len() == 1 => Value::Number(n[0]),
        (ValueKind::Color, M::Numbers(n)) if n.len() == 3 => Value::Color([n[0], n[1], n[2]]),
        (ValueKind::Channels(_) | ValueKind::Vector { .. }, M::Numbers(n)) => {
            Value::Numbers(n.clone())
        }
        (ValueKind::Choice { .. }, M::Text(t)) => Value::Choice(Arc::from(t.as_str())),
        (ValueKind::Text, M::Text(t)) => Value::Text(t.clone()),
        _ => return None,
    })
}

/// Value `v`, as a MaterialX node's input holds it, as MaterialX's, when
/// it is one: what a format writes of a node's value.
pub fn of_value(v: &Value) -> Option<kinogaki_mtlx::Value> {
    use kinogaki_mtlx::Value as M;
    Some(match v {
        Value::Bool(on) => M::Boolean(*on),
        Value::Integer(n) => M::Integer(*n),
        Value::Number(n) => M::Numbers(vec![*n]),
        Value::Color(c) => M::Numbers(c.to_vec()),
        Value::Numbers(n) => M::Numbers(n.clone()),
        Value::Choice(c) => M::Text(c.to_string()),
        Value::Text(t) => M::Text(t.clone()),
        _ => return None,
    })
}

/// A value of `kind` where MaterialX gives none: zeros, false, empty.
fn zero(kind: &ValueKind) -> Value {
    match kind {
        ValueKind::Toggle => Value::Bool(false),
        ValueKind::Integer { min, .. } => Value::Integer((*min).max(0)),
        ValueKind::Color => Value::Color([0.0; 3]),
        ValueKind::Channels(_) => Value::Numbers(vec![0.0; 4]),
        ValueKind::Vector { components, .. } => Value::Numbers(vec![0.0; *components as usize]),
        ValueKind::Choice { choices } => Value::Choice(
            choices
                .first()
                .map_or_else(|| Arc::from(""), |c| c.key.clone()),
        ),
        ValueKind::Text => Value::Text(String::new()),
        _ => Value::Number(0.0),
    }
}

/// What a UI calls definition `def`'s node: its [`LABELS`] entry, else
/// MaterialX's label, else its name in words.
fn label_of(def: &kinogaki_mtlx::NodeDef) -> String {
    let named = LABELS.iter().find(|(n, _)| *n == def.node);
    match (named, &def.label) {
        (Some((_, label)), _) => (*label).to_owned(),
        (None, Some(label)) => label.clone(),
        (None, None) => words(&def.node),
    }
}

/// The nodes a UI calls other than their names in words: names that run
/// words together, and names a host's own node has (the host's Image node
/// reads a file; MaterialX's `image` samples one). Their types keep
/// MaterialX's names.
const LABELS: &[(&str, &str)] = &[
    ("image", "Image Texture"),
    ("gltf_pbr", "glTF PBR"),
    ("gltf_image", "glTF Image"),
    ("gltf_colorimage", "glTF Color Image"),
    ("gltf_normalmap", "glTF Normal Map"),
    ("gltf_iridescence_thickness", "glTF Iridescence Thickness"),
    ("gltf_anisotropy_image", "glTF Anisotropy Image"),
    ("UsdPreviewSurface", "USD Preview Surface"),
    ("UsdUVTexture", "USD UV Texture"),
    ("UsdPrimvarReader", "USD Primvar Reader"),
    ("UsdTransform2d", "USD Transform 2D"),
    ("surfacematerial", "Surface Material"),
    ("volumematerial", "Volume Material"),
    ("tiledimage", "Tiled Image"),
    ("hextiledimage", "Hex Tiled Image"),
    ("hextilednormalmap", "Hex Tiled Normal Map"),
    ("latlongimage", "Lat-Long Image"),
    ("triplanarprojection", "Triplanar Projection"),
    ("texcoord", "Texture Coordinate"),
    ("geomcolor", "Geometric Color"),
    ("geompropvalue", "Geometric Property Value"),
    ("geompropvalueuniform", "Geometric Property Value Uniform"),
    ("viewdirection", "View Direction"),
    ("normalmap", "Normal Map"),
    ("heighttonormal", "Height to Normal"),
    ("absval", "Absolute Value"),
    ("dotproduct", "Dot Product"),
    ("crossproduct", "Cross Product"),
    ("invertmatrix", "Invert Matrix"),
    ("creatematrix", "Create Matrix"),
    ("transformpoint", "Transform Point"),
    ("transformvector", "Transform Vector"),
    ("transformnormal", "Transform Normal"),
    ("transformmatrix", "Transform Matrix"),
    ("rotate2d", "Rotate 2D"),
    ("rotate3d", "Rotate 3D"),
    ("place2d", "Place 2D"),
    ("safepower", "Safe Power"),
    ("ifgreater", "If Greater"),
    ("ifgreatereq", "If Greater or Equal"),
    ("ifequal", "If Equal"),
    ("ramplr", "Ramp Left Right"),
    ("ramptb", "Ramp Top Bottom"),
    ("ramp4", "Ramp 4"),
    ("splitlr", "Split Left Right"),
    ("splittb", "Split Top Bottom"),
    ("rgbtohsv", "RGB to HSV"),
    ("hsvtorgb", "HSV to RGB"),
    ("hsvadjust", "HSV Adjust"),
    ("colorcorrect", "Color Correct"),
    ("unpremult", "Unpremultiply"),
    ("premult", "Premultiply"),
    ("disjointover", "Disjoint Over"),
    ("randomfloat", "Random Float"),
    ("randomcolor", "Random Color"),
    ("cellnoise2d", "Cell Noise 2D"),
    ("cellnoise3d", "Cell Noise 3D"),
    ("worleynoise2d", "Worley Noise 2D"),
    ("worleynoise3d", "Worley Noise 3D"),
    ("unifiednoise2d", "Unified Noise 2D"),
    ("unifiednoise3d", "Unified Noise 3D"),
    ("noise2d", "Noise 2D"),
    ("noise3d", "Noise 3D"),
    ("fractal2d", "Fractal 2D"),
    ("fractal3d", "Fractal 3D"),
    ("flake2d", "Flake 2D"),
    ("flake3d", "Flake 3D"),
    ("trianglewave", "Triangle Wave"),
    ("facingratio", "Facing Ratio"),
    ("checkerboard", "Checkerboard"),
    ("tiledcircles", "Tiled Circles"),
    ("tiledcloverleafs", "Tiled Cloverleafs"),
    ("tiledhexagons", "Tiled Hexagons"),
    ("anisotropic_vdf", "Anisotropic VDF"),
    ("absorption_vdf", "Absorption VDF"),
    ("uniform_edf", "Uniform EDF"),
    ("conical_edf", "Conical EDF"),
    ("measured_edf", "Measured EDF"),
    ("generalized_schlick_edf", "Generalized Schlick EDF"),
    ("dielectric_bsdf", "Dielectric BSDF"),
    ("conductor_bsdf", "Conductor BSDF"),
    ("generalized_schlick_bsdf", "Generalized Schlick BSDF"),
    ("oren_nayar_diffuse_bsdf", "Oren-Nayar Diffuse BSDF"),
    ("burley_diffuse_bsdf", "Burley Diffuse BSDF"),
    ("sheen_bsdf", "Sheen BSDF"),
    ("subsurface_bsdf", "Subsurface BSDF"),
    ("translucent_bsdf", "Translucent BSDF"),
    ("chiang_hair_bsdf", "Chiang Hair BSDF"),
];

/// Node group `group` as a UI lists it (`texture2d` as Texture 2D).
fn family(group: &str) -> String {
    match group {
        "texture2d" => "Texture 2D".into(),
        "texture3d" => "Texture 3D".into(),
        "procedural2d" => "Procedural 2D".into(),
        "procedural3d" => "Procedural 3D".into(),
        "convolution2d" => "Convolution 2D".into(),
        "colortransform" => "Color Transform".into(),
        "pbr" => "PBR".into(),
        "npr" => "NPR".into(),
        "" => "Other".into(),
        other => words(other),
    }
}

/// `name` in words: split at underscores and before an uppercase letter that
/// follows a lowercase one, each word capitalized (`specular_IOR` as Specular
/// IOR, `diffuseColor` as Diffuse Color).
pub(crate) fn words(name: &str) -> String {
    let mut out = String::new();
    let mut previous: Option<char> = None;
    for ch in name.chars() {
        if ch == '_' {
            previous = None;
            continue;
        }
        let starts = match previous {
            None => true,
            Some(p) => p.is_ascii_lowercase() && ch.is_ascii_uppercase(),
        };
        if starts && !out.is_empty() {
            out.push(' ');
        }
        match starts {
            true => out.extend(ch.to_uppercase()),
            false => out.push(ch),
        }
        previous = Some(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinogaki_value::Unit;

    #[test]
    fn materialx_nodes_keep_their_names_and_meaning() {
        let image = def("ND_image_color3").unwrap();
        assert_eq!(image.id.as_str(), "kinogaki.mtlx.image");
        assert_eq!(
            (&*image.label, &*image.family),
            ("Image Texture", "Texture 2D")
        );
        assert_eq!(image.mtlx().unwrap().name, "ND_image_color3");
        // A file is an image wire alone, with no color space row of its own.
        assert!(matches!(
            image.find_input("file"),
            Some(Input::Wire {
                socket: Socket::Image,
                ..
            })
        ));
        assert!(image.find_input("filecolorspace").is_none());
        // An unwired texture coordinate reads the geometry's: a wire alone.
        assert!(matches!(
            image.find_input("texcoord"),
            Some(Input::Wire {
                socket: Socket::Vector2,
                ..
            })
        ));
        let wrap = image
            .find_input("uaddressmode")
            .and_then(Input::value)
            .unwrap();
        assert_eq!(wrap.value, Value::Choice(Arc::from("periodic")));
        // Shared, made once.
        assert!(Arc::ptr_eq(&image, &def("ND_image_color3").unwrap()));
        let usd = def("ND_UsdPreviewSurface_surfaceshader").unwrap();
        assert_eq!(
            (usd.id.as_str(), &*usd.label),
            ("kinogaki.mtlx.UsdPreviewSurface", "USD Preview Surface")
        );
        assert_eq!(
            usd.find_input("diffuseColor").unwrap().label(),
            "Diffuse Color"
        );
        let open_pbr = def("ND_open_pbr_surface_surfaceshader").unwrap();
        assert_eq!(&*open_pbr.label, "OpenPBR Surface");
        assert_eq!(open_pbr.outputs[0].socket, Socket::surface());
        let alpha = def("ND_gltf_pbr_surfaceshader").unwrap();
        let mode = alpha
            .find_input("alpha_mode")
            .and_then(Input::value)
            .unwrap();
        assert!(
            matches!(&mode.kind, ValueKind::Integer { min: 0, max: 2, menu } if menu[1] == "MASK")
        );
        assert!(def("ND_nothing").is_none());
    }

    #[test]
    fn lengths_in_the_scenes_units_are_lengths() {
        let unit = |signature: &str, input: &str| {
            let def = def(signature).unwrap();
            match &def.find_input(input).and_then(Input::value).unwrap().kind {
                ValueKind::Number(n) | ValueKind::Vector { number: n, .. } => n.unit.clone(),
                other => panic!("{input}: {other:?}"),
            }
        };
        let gltf = "ND_gltf_pbr_surfaceshader";
        assert_eq!(unit(gltf, "thickness"), Unit::Length);
        assert_eq!(unit(gltf, "attenuation_distance"), Unit::Length);
        assert_eq!(unit(gltf, "ior"), Unit::Plain);
        let open_pbr = "ND_open_pbr_surface_surfaceshader";
        assert_eq!(unit(open_pbr, "transmission_depth"), Unit::Length);
        // MaterialX's own distances: in the scene's units unless their
        // definition fixes one.
        assert_eq!(
            unit("ND_tiledimage_color3", "realworldtilesize"),
            Unit::Length
        );
        assert_eq!(
            unit("ND_dielectric_bsdf", "thinfilm_thickness"),
            Unit::Plain
        );
        assert_eq!(unit(gltf, "iridescence_thickness"), Unit::Plain);
    }

    #[test]
    fn every_node_has_a_label_of_its_own() {
        let mut seen = HashMap::new();
        for node in library().nodes() {
            let source = library().signatures(node).next().unwrap();
            if let Some(other) = seen.insert(label_of(source), node) {
                panic!("{node} and {other} are both {}", label_of(source));
            }
        }
    }

    #[test]
    fn names_read_as_words() {
        assert_eq!(words("specular_IOR"), "Specular IOR");
        assert_eq!(words("diffuseColor"), "Diffuse Color");
        assert_eq!(words("base_color"), "Base Color");
        assert_eq!(words("in1"), "In1");
    }
}
