use super::*;
use crate::graph::Binding;

#[test]
fn the_library_reads_every_definition() {
    let library = library();
    let defs: Vec<_> = library.defs().collect();
    // MaterialX 1.39.5's 806 definitions and Kinogaki's 13.
    assert_eq!(defs.len(), 806 + 13);
    let image = library.def("ND_image_color3").expect("stdlib's image");
    assert_eq!(
        (image.node.as_str(), image.group.as_str()),
        ("image", "texture2d")
    );
    assert_eq!(library.signatures("image").count(), 6);
    let file = image.input("file").unwrap();
    assert_eq!((file.ty.clone(), file.uniform), (Type::Filename, true));
    let wrap = image.input("uaddressmode").unwrap();
    assert_eq!(
        wrap.enumeration,
        ["constant", "clamp", "periodic", "mirror"]
    );
    let uv = image.input("texcoord").unwrap();
    assert_eq!((uv.geomprop.as_deref(), &uv.value), (Some("UV0"), &None));
    let open_pbr = library.def("ND_open_pbr_surface_surfaceshader").unwrap();
    assert_eq!(
        (open_pbr.version.as_deref(), open_pbr.default_version),
        (Some("1.1.1"), true)
    );
    assert_eq!(open_pbr.label.as_deref(), Some("OpenPBR Surface"));
    let geomprop = library.geomprop("UV0").unwrap();
    assert_eq!(
        (geomprop.node.as_str(), geomprop.index),
        ("texcoord", Some(0))
    );
    assert_eq!(
        library.geomprop("Nworld").unwrap().space.as_deref(),
        Some("world")
    );
}

#[test]
fn an_inheriting_definition_holds_its_parents_inputs_first() {
    let library = library();
    let parent = library.def("ND_gltf_image_color3_color3_1_0").unwrap();
    let child = library.def("ND_kinogaki_gltf_image_color3").unwrap();
    let names = |d: &NodeDef| d.inputs.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
    let mut expected = names(parent);
    expected.push("minfilter".into());
    assert_eq!(names(child), expected);
    // The child's filter takes an undefined one; its outputs are the parent's.
    let filter = child.input("filtertype").unwrap();
    assert_eq!(filter.enumeration[0], "");
    assert_eq!(child.outputs, parent.outputs);
    // MaterialX's own: USD's texture 2.2 takes 2.3's inputs, with its own
    // outputs.
    let older = library.def("ND_UsdUVTexture").unwrap();
    assert!(older.input("wrapS").is_some());
    assert_eq!(older.outputs.len(), 6);
}

#[test]
fn every_graph_implementation_resolves_and_type_checks() {
    let library = library();
    let mut graphs = 0;
    for def in library.defs() {
        let Some(source) = library.graph_source(def) else {
            continue;
        };
        let graph = crate::graph::resolve(library, def, source)
            .unwrap_or_else(|why| panic!("{}: {why}", def.name));
        assert_eq!(graph.outputs.len(), def.outputs.len(), "{}", def.name);
        // What the cache keeps is that graph.
        assert_eq!(library.implementation(def).as_deref(), Some(&graph));
        graphs += 1;
    }
    // MaterialX's 266 graph implementations (two by implementation
    // elements) and Kinogaki's 13.
    assert_eq!(graphs, 266 + 13);
    let leaf = library.def("ND_multiply_float").unwrap();
    assert!(
        library.implementation(leaf).is_none(),
        "a leaf is an engine's"
    );
}

#[test]
fn a_graph_binds_values_interfaces_and_other_nodes() {
    let library = library();
    let def = library
        .def("ND_kinogaki_phong_surface_surfaceshader")
        .unwrap();
    let graph = library.implementation(def).unwrap();
    let at = |name: &str| graph.nodes.iter().position(|n| n.name == name).unwrap();
    let surface = &graph.nodes[at("surface")];
    assert_eq!(surface.def.name, "ND_standard_surface_surfaceshader");
    let bound = |name: &str| &surface.inputs.iter().find(|(n, _)| n == name).unwrap().1;
    assert_eq!(bound("base"), &Binding::Value(Value::Numbers(vec![1.0])));
    assert_eq!(bound("specular_IOR"), &Binding::Interface("ior".into()));
    let roughness = Binding::Output {
        node: at("roughness"),
        output: "out".into(),
    };
    assert_eq!(bound("specular_roughness"), &roughness);
    assert_eq!(
        graph.outputs,
        [(
            "out".to_owned(),
            Binding::Output {
                node: at("surface"),
                output: "out".into()
            }
        )]
    );
}

#[test]
fn nodes_resolve_as_materialx_resolves_them() {
    let library = library();
    let resolved = |node, ty, inputs: &[(&str, &str)]| {
        library
            .resolve(node, ty, inputs, None)
            .map(|d| d.name.as_str())
    };
    assert_eq!(resolved("image", "color3", &[]), Some("ND_image_color3"));
    assert_eq!(
        resolved("multiply", "color3", &[("in2", "float")]),
        Some("ND_multiply_color3FA")
    );
    assert_eq!(
        resolved("multiply", "color3", &[("in2", "color3")]),
        Some("ND_multiply_color3")
    );
    // The default version, unless another is named.
    assert_eq!(
        resolved("standard_surface", "surfaceshader", &[]),
        Some("ND_standard_surface_surfaceshader")
    );
    let older = library.resolve("standard_surface", "surfaceshader", &[], Some("1.0.0"));
    assert_eq!(
        older.map(|d| d.name.as_str()),
        Some("ND_standard_surface_surfaceshader_100")
    );
    assert_eq!(resolved("image", "color3", &[("nope", "float")]), None);
}

/// MaterialX's own and Kinogaki's documents read in at most 10 ms (an M4
/// Max, release), once a process.
#[test]
#[ignore = "hand: a measurement, in release"]
fn hand_run_the_library_reads_in_10_ms() {
    let started = std::time::Instant::now();
    let library = Library::read(SOURCES).unwrap();
    let elapsed = started.elapsed();
    println!(
        "MTLX library read: {elapsed:?} for {} definitions",
        library.defs.len()
    );
    assert!(elapsed.as_secs_f64() <= 0.010, "{elapsed:?}");
}
