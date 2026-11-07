use super::*;
use crate::Engines;
use crate::material::mtlx::def;
use kinogaki_geometry::image::{ImageSource, Wrap};

/// A node feeding a surface: its signature, its output, its values and the
/// surface's input it feeds.
type Wired<'a> = (&'a str, &'a str, &'a [(&'a str, Value)], &'a str);

/// A material of `surface` (a signature) holding `values`, its `input` wired
/// from `upstream` (a signature, holding `held`, its image `wood.png` when it
/// reads one) when given.
fn material(surface: &str, values: &[(&str, Value)], wired: Option<Wired<'_>>) -> MaterialNetwork {
    let params = |v: &[(&str, Value)]| -> Parameters {
        v.iter()
            .map(|(n, v)| ((*n).to_owned(), v.clone()))
            .collect()
    };
    let mut built = MaterialNetwork::builder();
    let s = built.node(&def(surface).unwrap(), &params(values)).unwrap();
    if let Some((upstream, output, held, input)) = wired {
        let upstream = def(upstream).unwrap();
        let u = built.node(&upstream, &params(held)).unwrap();
        if upstream.find_input("file").is_some() {
            built
                .image(u, "file", ImageSource::path("wood.png"))
                .unwrap();
        }
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
        // Its constants fold away: an OpenPBR Surface alone, shown.
        let form = Engines::default()
            .standard(&material(surface, &extra, None))
            .unwrap();
        let mut nodes: Vec<&str> = (form.nodes().iter())
            .map(|n| n.def.mtlx().unwrap().node.as_str())
            .collect();
        nodes.sort_unstable();
        assert_eq!(nodes, ["open_pbr_surface", "surfacematerial"], "{surface}");
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
        let network = material(surface, &[], Some((texture, output, &[], input)));
        let display = shown(&network);
        let texture = display
            .texture
            .clone()
            .unwrap_or_else(|| panic!("{surface}: {display:?}"));
        assert_eq!(
            (texture.image.name(), &*texture.uv),
            ("wood.png", "uv"),
            "{surface}"
        );
    }
    // glTF's placement: its transform scales about its pivot, its wrap and
    // filter as its sampler says.
    let held = [
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
    assert_eq!(texture.sampling.filter, SampleFilter::Nearest);
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

/// The OpenPBR Surface values of `values`' USD Preview Surface's standard
/// form, its constants folded.
fn preview_form(values: &[(&str, Value)]) -> Parameters {
    let network = material("ND_UsdPreviewSurface_surfaceshader", values, None);
    let form = Engines::default().standard(&network).unwrap();
    let surface = form.surface().unwrap();
    assert_eq!(
        form.nodes().len(),
        2,
        "folded to a surface and its material"
    );
    form.nodes()[surface as usize].values.clone()
}

#[test]
fn a_preview_surface_translates_input_by_input() {
    let number = |v: &Parameters, name: &str| v[name].number().unwrap();
    let color = |v: &Parameters, name: &str| v[name].color().unwrap();
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    // The metallic workflow: its colors, metal, roughness, index and coat.
    let metal = preview_form(&[
        ("diffuseColor", Value::Color(ORANGE)),
        ("emissiveColor", Value::Color([0.1, 0.2, 0.3])),
        ("metallic", Value::Number(0.25)),
        ("roughness", Value::Number(0.4)),
        ("ior", Value::Number(1.8)),
        ("clearcoat", Value::Number(0.6)),
        ("clearcoatRoughness", Value::Number(0.2)),
    ]);
    assert_eq!(color(&metal, "base_color"), ORANGE);
    assert_eq!(color(&metal, "emission_color"), [0.1, 0.2, 0.3]);
    assert_eq!(color(&metal, "specular_color"), [1.0; 3]);
    let numbers = [
        ("base_metalness", 0.25),
        ("specular_weight", 1.0),
        ("specular_roughness", 0.4),
        ("specular_ior", 1.8),
        ("coat_weight", 0.6),
        ("coat_roughness", 0.2),
        ("coat_ior", 1.8),
        ("coat_darkening", 0.0),
        ("emission_luminance", 1.0),
        ("transmission_weight", 0.0),
        ("geometry_opacity", 1.0),
    ];
    for (name, expected) in numbers {
        assert!(near(number(&metal, name), expected), "{name}: {metal:?}");
    }
    // The specular workflow: its color is the reflectance at normal
    // incidence (an index of 1.5 reflects 0.04), metallic ignored.
    let specular = preview_form(&[
        ("useSpecularWorkflow", Value::Integer(1)),
        ("specularColor", Value::Color([0.4, 0.2, 0.1])),
        ("metallic", Value::Number(1.0)),
    ]);
    assert!(near(number(&specular, "base_metalness"), 0.0));
    assert!(near(number(&specular, "specular_weight"), 0.4 / 0.04));
    let tint = color(&specular, "specular_color");
    assert!(
        (tint.iter().zip([1.0, 0.5, 0.25])).all(|(a, b)| near(*a, b)),
        "{tint:?}"
    );
    // Opacity: transparent transmits, presence fades, a threshold cuts.
    let cases = [
        (0, 0.0, 0.3, 0.7, 1.0),
        (1, 0.0, 0.3, 0.0, 0.3),
        (0, 0.5, 0.3, 0.0, 0.0),
        (1, 0.5, 0.6, 0.0, 1.0),
        (0, 0.5, 0.5, 0.0, 1.0),
    ];
    for (mode, threshold, opacity, transmission, shown) in cases {
        let form = preview_form(&[
            ("opacityMode", Value::Integer(mode)),
            ("opacityThreshold", Value::Number(threshold)),
            ("opacity", Value::Number(opacity)),
        ]);
        let got = (
            number(&form, "transmission_weight"),
            number(&form, "geometry_opacity"),
        );
        assert!(
            near(got.0, transmission) && near(got.1, shown),
            "mode {mode}, threshold {threshold}, opacity {opacity}: {got:?}"
        );
    }
}

/// A file format writing glTF PBR reads OpenPBR Surface translated into it,
/// through Standard Surface: its constants fold into glTF PBR's, the
/// roughness through MaterialX's `roughness_anisotropy` included.
#[test]
fn openpbr_translates_into_gltf_pbr_as_constants() {
    let open = [
        ("base_color", Value::Color(ORANGE)),
        ("base_metalness", Value::Number(1.0)),
        ("specular_roughness", Value::Number(0.25)),
    ];
    let network = material("ND_open_pbr_surface_surfaceshader", &open, None);
    let gltf = network.translated("gltf_pbr").expect("translates");
    let mut nodes: Vec<&str> = (gltf.nodes().iter())
        .map(|n| n.def.mtlx().unwrap().node.as_str())
        .collect();
    nodes.sort_unstable();
    assert_eq!(nodes, ["gltf_pbr", "surfacematerial"], "folded");
    let values = &gltf.nodes()[gltf.surface().unwrap() as usize].values;
    let color = values["base_color"].color().unwrap();
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(
        (color.iter().zip(ORANGE)).all(|(a, b)| near(*a, b)),
        "{color:?}"
    );
    assert_eq!(values.get("metallic").and_then(Value::number), Some(1.0));
    let roughness = values.get("roughness").and_then(Value::number).unwrap();
    assert!(near(roughness, 0.25), "{roughness}");
}
