use super::*;
use crate::Engines;
use crate::material::mtlx::def;
use kinogaki_geometry::image::{Filter, Wrap};

/// A node feeding a surface: its signature, its output, its values and the
/// surface's input it feeds.
type Wired<'a> = (&'a str, &'a str, &'a [(&'a str, Value)], &'a str);

/// A material of `surface` (a signature) holding `values`, its `input` wired
/// from `upstream` (a signature, holding `held`) when given.
fn material(surface: &str, values: &[(&str, Value)], wired: Option<Wired<'_>>) -> MaterialNetwork {
    let params = |v: &[(&str, Value)]| -> Parameters {
        v.iter()
            .map(|(n, v)| ((*n).to_owned(), v.clone()))
            .collect()
    };
    let mut built = MaterialNetwork::builder();
    let s = built.node(&def(surface).unwrap(), &params(values)).unwrap();
    if let Some((upstream, output, held, input)) = wired {
        let u = built.node(&def(upstream).unwrap(), &params(held)).unwrap();
        built.link(u, output, s, input).unwrap();
    }
    let m = built
        .node(&def("ND_surfacematerial").unwrap(), &Parameters::new())
        .unwrap();
    built.link(s, "out", m, "surfaceshader").unwrap();
    built.build(m).unwrap()
}

/// What the viewport shows of `network`: its standard form's display.
fn shown(network: &MaterialNetwork) -> MaterialDisplay {
    let standard = Engines::default()
        .standard(network)
        .expect("a standard form");
    display(&standard)
}

const ORANGE: [f64; 3] = [0.9, 0.4, 0.1];

#[test]
fn each_model_shows_its_constant_base_color() {
    let orange = Value::Color(ORANGE);
    let cases = [
        ("ND_open_pbr_surface_surfaceshader", "base_color"),
        ("ND_standard_surface_surfaceshader", "base_color"),
        ("ND_gltf_pbr_surfaceshader", "base_color"),
        ("ND_UsdPreviewSurface_surfaceshader", "diffuseColor"),
        ("ND_kinogaki_phong_surface_surfaceshader", "diffuse_color"),
        ("ND_surface_unlit", "emission_color"),
    ];
    for (surface, input) in cases {
        let extra = match surface {
            "ND_standard_surface_surfaceshader" => {
                vec![(input, orange.clone()), ("base", Value::Number(1.0))]
            }
            _ => vec![(input, orange.clone())],
        };
        let display = shown(&material(surface, &extra, None));
        let near =
            (display.color.iter().zip(ORANGE)).all(|(a, b)| (f64::from(*a) - b).abs() < 1e-6);
        assert!(near, "{surface}: {:?}", display.color);
        assert_eq!(
            (display.texture, display.primvar),
            (None, None),
            "{surface}"
        );
    }
}

#[test]
fn an_image_shows_through_any_model_with_its_placement() {
    let file = ("file", Value::Text("wood.png".into()));
    let cases = [
        (
            "ND_open_pbr_surface_surfaceshader",
            "ND_image_color3",
            "out",
            "base_color",
        ),
        (
            "ND_gltf_pbr_surfaceshader",
            "ND_gltf_colorimage",
            "outcolor",
            "base_color",
        ),
        (
            "ND_UsdPreviewSurface_surfaceshader",
            "ND_UsdUVTexture_23",
            "rgb",
            "diffuseColor",
        ),
        (
            "ND_standard_surface_surfaceshader",
            "ND_tiledimage_color3",
            "out",
            "base_color",
        ),
    ];
    for (surface, texture, output, input) in cases {
        let network = material(
            surface,
            &[],
            Some((texture, output, std::slice::from_ref(&file), input)),
        );
        let display = shown(&network);
        let texture = display
            .texture
            .clone()
            .unwrap_or_else(|| panic!("{surface}: {display:?}"));
        assert_eq!(
            (&*texture.file, &*texture.uv),
            ("wood.png", "uv"),
            "{surface}"
        );
    }
    // glTF's placement: its transform scales about its pivot, its wrap and
    // filter as its sampler says.
    let held = [
        file.clone(),
        ("scale", Value::Numbers(vec![2.0, 4.0])),
        ("uaddressmode", Value::Choice(Arc::from("clamp"))),
        ("vaddressmode", Value::Choice(Arc::from("mirror"))),
        ("filtertype", Value::Choice(Arc::from("closest"))),
    ];
    // (glTF's color image fixes its own wraps at periodic, as MaterialX 1.39.5
    // defines it; its image doesn't.)
    let gltf = material(
        "ND_gltf_pbr_surfaceshader",
        &[],
        Some((
            "ND_gltf_image_color3_color3_1_0",
            "out",
            &held,
            "base_color",
        )),
    );
    let texture = shown(&gltf).texture.expect("the image");
    assert_eq!(texture.sampling.wrap, [Wrap::Clamp, Wrap::MirrorRepeat]);
    assert_eq!(texture.sampling.filter, Filter::Nearest);
    // glTF scales coordinates by its scale about (0, 1): u 0.5 reads 1, v 0.5 reads -1.
    let at = texture.at([0.5, 0.5]);
    assert!(
        (at[0] - 1.0).abs() < 1e-5 && (at[1] + 1.0).abs() < 1e-5,
        "{at:?}"
    );
}

#[test]
fn a_vertex_color_shows_as_its_primvar() {
    let network = material(
        "ND_open_pbr_surface_surfaceshader",
        &[],
        Some(("ND_geomcolor_color3", "out", &[], "base_color")),
    );
    let display = shown(&network);
    assert_eq!(display.primvar.as_deref(), Some("Cd"));
    let named = material(
        "ND_gltf_pbr_surfaceshader",
        &[],
        Some((
            "ND_geompropvalue_color3",
            "out",
            &[("geomprop", Value::Text("tint".into()))],
            "base_color",
        )),
    );
    assert_eq!(shown(&named).primvar.as_deref(), Some("tint"));
}

#[test]
fn open_pbr_makes_a_shown_surface() {
    let network = open_pbr(&[("base_color".to_owned(), Value::Color(ORANGE))].into()).unwrap();
    let surface = network.surface().unwrap();
    assert_eq!(
        network.nodes()[surface as usize].def.id.as_str(),
        "kinogaki.mtlx.open_pbr_surface"
    );
    assert_eq!(
        network.output().unwrap().def.id.as_str(),
        "kinogaki.mtlx.surfacematerial"
    );
    assert!(open_pbr(&[("nope".to_owned(), Value::Number(1.0))].into()).is_err());
}
