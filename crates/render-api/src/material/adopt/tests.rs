use super::super::mtlx::def;
use super::super::standard::open_pbr;
use crate::{Capabilities, EngineDescription, Engines, MaterialNetwork, Socket};
use kinogaki_mtlx::library;
use kinogaki_value::{EngineId, Parameters, Value};
use std::sync::Arc;

/// An engine implementing `nodes` (node names or signatures).
fn engine(id: &'static str, nodes: &[&str]) -> EngineDescription {
    let capabilities = Capabilities {
        standard_nodes: nodes.iter().map(|n| Arc::from(*n)).collect(),
        ..Capabilities::default()
    };
    EngineDescription::builder(EngineId::declared(id), id)
        .capabilities(capabilities)
        .build()
}

/// Every node MaterialX defines without a graph: an engine implementing them
/// all renders every model, as MaterialX means.
fn leaves() -> Vec<&'static str> {
    let library = library();
    let mut leaves: Vec<&'static str> = Vec::new();
    for node in library.nodes() {
        for d in library.signatures(node) {
            if library.implementation(d).is_none() {
                leaves.push(Box::leak(d.name.clone().into_boxed_str()));
            }
        }
    }
    leaves
}

/// A material: `surface` (a signature) holding `values`, shown through a
/// Surface Material.
fn material(surface: &str, values: &[(&str, Value)]) -> MaterialNetwork {
    let values: Parameters = values
        .iter()
        .map(|(n, v)| ((*n).to_owned(), v.clone()))
        .collect();
    let mut built = MaterialNetwork::builder();
    let s = built.node(&def(surface).unwrap(), &values).unwrap();
    let m = built
        .node(&def("ND_surfacematerial").unwrap(), &Parameters::new())
        .unwrap();
    built.link(s, "out", m, "surfaceshader").unwrap();
    built.build(m).unwrap()
}

/// The nodes of `network`, by MaterialX node, sorted.
fn nodes(network: &MaterialNetwork) -> Vec<String> {
    let mut names: Vec<String> = (network.nodes().iter())
        .map(|n| {
            n.def
                .mtlx()
                .map_or_else(|| n.def.id.to_string(), |d| d.node.clone())
        })
        .collect();
    names.sort();
    names
}

#[test]
fn a_node_the_engine_implements_passes_as_it_is() {
    let renders = engine("toy.open.engine", &["open_pbr_surface", "surfacematerial"]);
    let engines = Engines::new(vec![renders.clone()]);
    let network = material(
        "ND_open_pbr_surface_surfaceshader",
        &[("base_color", Value::Color([0.2, 0.3, 0.4]))],
    );
    let adopted = engines.adopt(&network, renders.id());
    assert_eq!(adopted, network);
    assert!(adopted.cut().is_empty());
}

#[test]
fn a_model_expands_for_an_engine_of_every_leaf_and_translates_for_one_of_openpbr() {
    let gltf = material(
        "ND_gltf_pbr_surfaceshader",
        &[
            ("base_color", Value::Color([0.9, 0.5, 0.1])),
            ("metallic", Value::Number(0.0)),
            ("roughness", Value::Number(0.4)),
        ],
    );
    // An engine of MaterialX's leaves: glTF PBR as its definition, exact.
    let leaves = engine("toy.leaves.engine", &leaves());
    let engines = Engines::new(vec![leaves.clone()]);
    let expanded = engines.adopt(&gltf, leaves.id());
    assert!(expanded.cut().is_empty(), "{:?}", expanded.cut());
    let made = nodes(&expanded);
    assert!(!made.contains(&"gltf_pbr".to_owned()), "expanded");
    assert!(made.contains(&"dielectric_bsdf".to_owned()) && made.contains(&"surface".to_owned()));
    // Its graph's constants folded: no node of values alone that yields a
    // value is left (a sheen of no color divides by zero: it stays).
    for (at, node) in expanded.nodes().iter().enumerate() {
        let driven = expanded.links().iter().any(|l| l.to == at as u32);
        let def = node.def.mtlx().unwrap();
        let values = |name: &str| node.values.get(name).and_then(super::super::mtlx::of_value);
        assert!(
            driven || def.evaluate(&values).is_none(),
            "{} left",
            def.name
        );
    }
    // An engine of OpenPBR alone: glTF PBR translated, its values carried and
    // the translation folded away.
    let open = engine("toy.open.engine", &["open_pbr_surface", "surfacematerial"]);
    let engines = Engines::new(vec![open.clone()]);
    let translated = engines.adopt(&gltf, open.id());
    assert!(translated.cut().is_empty(), "{:?}", translated.cut());
    assert_eq!(nodes(&translated), ["open_pbr_surface", "surfacematerial"]);
    let surface = &translated.nodes()[translated.surface().unwrap() as usize];
    let held = |name: &str| surface.values[name].clone();
    assert_eq!(held("base_color"), Value::Color([0.9, 0.5, 0.1]));
    assert_eq!(held("base_metalness"), Value::Number(0.0));
    assert_eq!(held("specular_roughness"), Value::Number(0.4));
    assert_eq!(held("geometry_opacity"), Value::Number(1.0), "OPAQUE");
}

#[test]
fn standard_surface_translates_by_materialx_own_graph() {
    let standard = material(
        "ND_standard_surface_surfaceshader",
        &[
            ("base", Value::Number(0.5)),
            ("base_color", Value::Color([1.0, 0.5, 0.25])),
            ("metalness", Value::Number(1.0)),
        ],
    );
    let open = engine("toy.open.engine", &["open_pbr_surface", "surfacematerial"]);
    let engines = Engines::new(vec![open.clone()]);
    let translated = engines.adopt(&standard, open.id());
    assert!(translated.cut().is_empty(), "{:?}", translated.cut());
    let surface = &translated.nodes()[translated.surface().unwrap() as usize];
    assert_eq!(surface.values["base_weight"], Value::Number(0.5));
    assert_eq!(surface.values["base_color"], Value::Color([1.0, 0.5, 0.25]));
    // MaterialX's graph: a metal's specular weight is 1, whatever `specular` says.
    assert_eq!(surface.values["specular_weight"], Value::Number(1.0));
}

#[test]
fn a_conditional_of_values_passes_the_input_it_chooses_on() {
    // Standard Surface's `specular` from an image, on a dielectric: MaterialX's
    // translation chooses it by `metalness` (`ifgreater`), which the engine
    // doesn't implement; the choice is made here and the image drives
    // OpenPBR's specular weight.
    let mut built = MaterialNetwork::builder();
    let image = built
        .node(&def("ND_image_float").unwrap(), &Parameters::new())
        .unwrap();
    let metalness = [("metalness".to_owned(), Value::Number(0.0))].into();
    let s = built
        .node(
            &def("ND_standard_surface_surfaceshader").unwrap(),
            &metalness,
        )
        .unwrap();
    built.link(image, "out", s, "specular").unwrap();
    let network = built.build(s).unwrap();
    let open = engine(
        "toy.open.engine",
        &["open_pbr_surface", "surfacematerial", "ND_image_float"],
    );
    let adopted = Engines::new(vec![open.clone()]).adopt(&network, open.id());
    assert!(adopted.cut().is_empty(), "{:?}", adopted.cut());
    assert_eq!(nodes(&adopted), ["image", "open_pbr_surface"]);
    let [link] = adopted.links() else {
        panic!("one wire: {:?}", adopted.links());
    };
    assert_eq!((&*link.output, &*link.input), ("out", "specular_weight"));
}

#[test]
fn what_no_way_reaches_is_cut_and_named_once() {
    let open = engine("toy.open.engine", &["open_pbr_surface", "surfacematerial"]);
    let engines = Engines::new(vec![open.clone()]);
    // A hexagonal tiling into the base color: the engine has no way to it.
    let mut built = MaterialNetwork::builder();
    let hex = def("ND_hextiledimage_color3").unwrap();
    let (h1, h2) = (
        built.node(&hex, &Parameters::new()).unwrap(),
        built.node(&hex, &Parameters::new()).unwrap(),
    );
    let mix = def("ND_mix_color3").unwrap();
    let x = built.node(&mix, &Parameters::new()).unwrap();
    let s = built
        .node(
            &def("ND_open_pbr_surface_surfaceshader").unwrap(),
            &Parameters::new(),
        )
        .unwrap();
    built.link(h1, "out", x, "fg").unwrap();
    built.link(h2, "out", x, "bg").unwrap();
    built.link(x, "out", s, "base_color").unwrap();
    let network = built.build(s).unwrap();
    let adopted = engines.adopt(&network, open.id());
    assert_eq!(nodes(&adopted), ["open_pbr_surface"]);
    let cut: Vec<&str> = adopted.cut().iter().map(|c| &**c).collect();
    assert_eq!(cut, ["hextiledimage"], "once, however many");
    // The mix holds its own values once the tilings go, and folds into the
    // base color: its defaults' mix, black.
    assert_eq!(
        adopted.nodes()[0].values["base_color"],
        Value::Color([0.0; 3])
    );
}

#[test]
fn a_surface_with_no_way_renders_the_default() {
    let nothing = engine("toy.none.engine", &[]);
    let engines = Engines::new(vec![nothing.clone()]);
    let network = material("ND_gltf_pbr_surfaceshader", &[]);
    let adopted = engines.adopt(&network, nothing.id());
    assert!(adopted.output().is_none(), "the engine's default material");
    assert!(!adopted.cut().is_empty());
}

#[test]
fn an_unconnected_geometric_input_reads_its_property_through_an_expansion() {
    // glTF's image reads UV0 unconnected; its graph's place2d takes a value
    // there, so the expansion reads the coordinates in its place.
    let mut built = MaterialNetwork::builder();
    let image = built
        .node(
            &def("ND_gltf_image_color3_color3_1_0").unwrap(),
            &Parameters::new(),
        )
        .unwrap();
    let s = built
        .node(
            &def("ND_open_pbr_surface_surfaceshader").unwrap(),
            &Parameters::new(),
        )
        .unwrap();
    built.link(image, "out", s, "base_color").unwrap();
    let network = built.build(s).unwrap();
    let leaves = engine("toy.leaves.engine", &leaves());
    let expanded = Engines::new(vec![leaves.clone()]).adopt(&network, leaves.id());
    assert!(
        nodes(&expanded).contains(&"texcoord".to_owned()),
        "{:?}",
        nodes(&expanded)
    );
    assert!(nodes(&expanded).contains(&"image".to_owned()));
}

#[test]
fn an_image_wired_into_a_graph_defined_texture_reaches_the_image_inside() {
    // glTF's normal map is a graph around MaterialX's image: expanded, its
    // image reads what was wired into the normal map, as data.
    use kinogaki_geometry::image::ImageSource;
    let mut built = MaterialNetwork::builder();
    let map = built
        .node(
            &def("ND_gltf_normalmap_vector3_1_0").unwrap(),
            &Parameters::new(),
        )
        .unwrap();
    built
        .image(map, "file", ImageSource::path("bumps.png"))
        .unwrap();
    let s = built
        .node(
            &def("ND_open_pbr_surface_surfaceshader").unwrap(),
            &Parameters::new(),
        )
        .unwrap();
    built.link(map, "out", s, "geometry_normal").unwrap();
    let network = built.build(s).unwrap();
    let leaves = engine("toy.leaves.engine", &leaves());
    let expanded = Engines::new(vec![leaves.clone()]).adopt(&network, leaves.id());
    assert!(expanded.cut().is_empty(), "{:?}", expanded.cut());
    let read: Vec<_> = (expanded.nodes().iter())
        .filter(|n| n.def.mtlx().is_some_and(|d| d.node == "image"))
        .map(|n| (n.images["file"].name(), n.reads_data("file")))
        .collect();
    assert_eq!(read, [("bumps.png", true)]);
}

#[test]
fn another_engines_material_goes_as_its_standard_form() {
    use crate::material::{Input, NodeDef, Output};
    use crate::{MaterialRules, Parameter};
    use kinogaki_value::{NodeTypeId, ValueKind};
    /// A toy engine's paint: its standard form an OpenPBR Surface of its color.
    struct Paint;
    impl MaterialRules for Paint {
        fn standard(&self, own: &MaterialNetwork) -> Option<MaterialNetwork> {
            let color = own.nodes()[own.surface()? as usize]
                .values
                .get("color")?
                .clone();
            open_pbr(&[("base_color".to_owned(), color)].into()).ok()
        }
    }
    let paint = NodeDef::new(
        NodeTypeId::declared("toy.paint.surface"),
        "Paint",
        "Surface",
    )
    .input(Input::Setting(Parameter::new(
        "color",
        "Color",
        ValueKind::Color,
        [0.5; 3],
    )))
    .output(Output::new("out", "Out", Socket::surface()));
    let painter = EngineDescription::builder(EngineId::declared("toy.paint.engine"), "Paint")
        .materials(vec![paint], Arc::new(Paint))
        .build();
    let open = engine("toy.open.engine", &["open_pbr_surface", "surfacematerial"]);
    let engines = Engines::new(vec![painter.clone(), open.clone()]);
    let mut built = MaterialNetwork::builder();
    let values = [("color".to_owned(), Value::Color([0.1, 0.2, 0.3]))].into();
    let s = built
        .node(painter.node_defs().first().unwrap(), &values)
        .unwrap();
    let own = built.build(s).unwrap();
    assert_eq!(
        engines.adopt(&own, painter.id()),
        own,
        "its own engine takes it as it is"
    );
    assert_eq!(engines.owner(&own).map(|e| e.id()), Some(painter.id()));
    let theirs = engines.adopt(&own, open.id());
    assert_eq!(nodes(&theirs), ["open_pbr_surface", "surfacematerial"]);
    assert_eq!(
        theirs.nodes()[theirs.surface().unwrap() as usize].values["base_color"],
        Value::Color([0.1, 0.2, 0.3])
    );
}

/// Adopting a material of 50 nodes takes at most 0.5 ms (an M4 Max,
/// release); expanding glTF PBR at most 1 ms.
#[test]
#[ignore = "hand: a measurement, in release"]
fn hand_run_adoption_is_quick() {
    let leaves = engine("toy.leaves.engine", &leaves());
    let open = engine(
        "toy.open.engine",
        &[
            "open_pbr_surface",
            "surfacematerial",
            "multiply",
            "image",
            "texcoord",
        ],
    );
    let engines = Engines::new(vec![leaves.clone(), open.clone()]);
    let gltf = material("ND_gltf_pbr_surfaceshader", &[]);
    let _ = engines.adopt(&gltf, leaves.id());
    let started = std::time::Instant::now();
    let expanded = engines.adopt(&gltf, leaves.id());
    let expanding = started.elapsed();
    println!(
        "MTLX expanding glTF PBR: {expanding:?}, {} nodes",
        expanded.nodes().len()
    );
    // Fifty nodes: a chain of image multiplications into OpenPBR.
    let mut built = MaterialNetwork::builder();
    let image = def("ND_image_color3").unwrap();
    let multiply = def("ND_multiply_color3").unwrap();
    let mut last = built.node(&image, &Parameters::new()).unwrap();
    for _ in 0..24 {
        let i = built.node(&image, &Parameters::new()).unwrap();
        let m = built.node(&multiply, &Parameters::new()).unwrap();
        built.link(last, "out", m, "in1").unwrap();
        built.link(i, "out", m, "in2").unwrap();
        last = m;
    }
    let s = built
        .node(
            &def("ND_open_pbr_surface_surfaceshader").unwrap(),
            &Parameters::new(),
        )
        .unwrap();
    built.link(last, "out", s, "base_color").unwrap();
    let fifty = built.build(s).unwrap();
    let started = std::time::Instant::now();
    let adopted = engines.adopt(&fifty, open.id());
    let adopting = started.elapsed();
    println!(
        "MTLX adopting {} nodes: {adopting:?}",
        adopted.nodes().len()
    );
    assert!(expanding.as_secs_f64() <= 0.001 && adopting.as_secs_f64() <= 0.0005);
}
