//! A document's materials as flat networks (MaterialX's own test documents,
//! `tests/documents/read`), a document written of materials reading back as
//! them, and includes read.
use kinogaki_mtlx::{Bound, Document, Included, Material, Value};
use std::collections::HashMap;
use std::path::Path;

fn document(name: &str) -> Document {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/documents/read")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap();
    text.parse().unwrap_or_else(|why| panic!("{name}: {why}"))
}

/// A node as compared: its name, definition and inputs, each in words.
type Named = (String, String, Vec<(String, String)>);

/// `m`'s nodes by name: each one's definition and inputs, a wire as the
/// name of the node and output it reads, numbers at MaterialX's precision.
fn named(m: &Material) -> Vec<Named> {
    let mut nodes: Vec<_> = (m.nodes.iter())
        .map(|n| {
            let inputs = (n.inputs.iter())
                .map(|(input, bound)| {
                    let held = match bound {
                        Bound::Value { value, colorspace } => format!("{value} {colorspace:?}"),
                        Bound::Output { node, output } => {
                            format!("<- {}.{output}", m.nodes[*node].name)
                        }
                    };
                    (input.clone(), held)
                })
                .collect();
            (n.name.clone(), n.def.name.clone(), inputs)
        })
        .collect();
    nodes.sort();
    nodes
}

/// `m`'s node `name`.
fn node<'m>(m: &'m Material, name: &str) -> &'m kinogaki_mtlx::MaterialNode {
    (m.nodes.iter())
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node {name}"))
}

/// What `m`'s node `name` holds at `input`.
fn input<'m>(m: &'m Material, name: &str, input: &str) -> &'m Bound {
    let found = node(m, name).inputs.iter().find(|(i, _)| i == input);
    &found.unwrap_or_else(|| panic!("{name} holds no {input}")).1
}

#[test]
fn a_document_s_own_definition_flattens_into_the_material_reading_it() {
    let (materials, said) = document("definition_with_enum_interface.mtlx").materials();
    assert!(said.is_empty(), "{said:?}");
    let [m] = &materials[..] else {
        panic!("one material")
    };
    assert_eq!(m.name, "Mtl");
    let names: Vec<&str> = m.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(
        names,
        ["myPos", "extract", "Srf", "Mtl"],
        "in the document's order"
    );
    let space = input(m, "myPos", "space");
    assert!(
        matches!(space, Bound::Value { value: Value::Text(t), .. } if t == "object"),
        "{space:?}"
    );
    let read = input(m, "extract", "in");
    assert_eq!(
        read,
        &Bound::Output {
            node: 0,
            output: "out".into()
        }
    );
    assert_eq!(m.material, 3);
}

#[test]
fn node_graphs_a_material_reads_from_are_the_material_s() {
    let (materials, said) = document("surfacematerial_with_graph.mtlx").materials();
    assert!(said.is_empty(), "{said:?}");
    let names: Vec<&str> = materials.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "M_Blue",
            "M_Magenta",
            "M_Orange",
            "M_EmptySurface",
            "M_EmptyVolume"
        ]
    );
    let blue = &materials[0];
    let surface = input(blue, "M_Blue", "surfaceshader");
    assert!(
        matches!(surface, Bound::Output { node, .. } if blue.nodes[*node].name == "standard_surface")
    );
    let shift = input(blue, "M_Blue", "displacementshader");
    assert!(
        matches!(shift, Bound::Output { node, .. } if blue.nodes[*node].def.node == "displacement")
    );
    // A graph's default output is its first.
    let magenta = &materials[1];
    let read = input(magenta, "M_Magenta", "surfaceshader");
    assert!(
        matches!(read, Bound::Output { node, .. } if magenta.nodes[*node].name == "standard_surface1")
    );
    // An empty surface reads nothing.
    assert!(materials[3].nodes[materials[3].material].inputs.is_empty());
}

#[test]
fn files_keep_their_prefix_and_values_their_color_space() {
    let (materials, _) = document("standard_surface_brass_tiled.mtlx").materials();
    let m = &materials[0];
    let file = |name: &str| input(m, name, "file").clone();
    assert_eq!(
        file("image_color"),
        Bound::Value {
            value: Value::Text("../../../Images/brass_color.jpg".into()),
            colorspace: Some("srgb_texture".into()),
        }
    );
    // The document's color space where the file says none.
    let rough = file("image_roughness");
    assert!(
        matches!(rough, Bound::Value { colorspace: Some(ref c), .. } if c == "lin_rec709"),
        "{rough:?}"
    );
    let base = input(m, "SR_brass1", "base_color").clone();
    assert!(
        matches!(base, Bound::Value { colorspace: Some(ref c), .. } if c == "lin_rec709"),
        "{base:?}"
    );
    let metal = input(m, "SR_brass1", "metalness").clone();
    assert!(
        matches!(
            metal,
            Bound::Value {
                colorspace: None,
                ..
            }
        ),
        "a float has no color space"
    );
}

#[test]
fn materials_written_as_a_document_read_back_as_themselves() {
    for name in [
        "surfacematerial_with_graph.mtlx",
        "standard_surface_brass_tiled.mtlx",
        "definition_with_enum_interface.mtlx",
        "gltf_pbr_boombox.mtlx",
        "multioutput.mtlx",
        "usd_preview_surface_brass_tiled.mtlx",
    ] {
        let (materials, _) = document(name).materials();
        let written = Document::of_materials(&materials, "lin_rec709");
        let again: Document = written.to_string().parse().unwrap();
        let (back, said) = again.materials();
        assert!(said.is_empty(), "{name}: {said:?}");
        assert_eq!(materials.len(), back.len(), "{name}");
        for (a, b) in materials.iter().zip(&back) {
            assert_eq!(
                (&a.name, a.double_sided),
                (&b.name, b.double_sided),
                "{name}"
            );
            assert_eq!(named(a), named(b), "{name}: {}", a.name);
        }
    }
}

#[test]
fn kinogaki_s_definitions_travel_with_a_document_using_them() {
    let text = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <kinogaki_phong_surface name="phong" type="surfaceshader">
    <input name="diffuse_color" type="color3" value="0.5, 0.25, 0" />
  </kinogaki_phong_surface>
  <surfacematerial name="Old Paint" type="material" kinogaki:doublesided="false">
    <input name="surfaceshader" type="surfaceshader" nodename="phong" />
  </surfacematerial>
</materialx>
"#;
    let document: Document = text.parse().unwrap();
    let (materials, _) = document.materials();
    assert!(!materials[0].double_sided);
    let written = Document::of_materials(&materials, "lin_rec709").to_string();
    assert!(
        written.contains("<nodedef name=\"ND_kinogaki_phong_surface_surfaceshader\""),
        "{written}"
    );
    assert!(written.contains("<nodegraph name=\"NG_kinogaki_phong_surface_surfaceshader\""));
    assert!(
        written.contains("name=\"Old_Paint\"") && written.contains("uiname=\"Old Paint\""),
        "{written}"
    );
    let (back, said) = written.parse::<Document>().unwrap().materials();
    assert!(said.is_empty(), "{said:?}");
    // Kinogaki's definitions in a document are the library's: still a node.
    assert_eq!(back[0].label.as_deref(), Some("Old Paint"));
    assert!(
        back[0]
            .nodes
            .iter()
            .any(|n| n.def.node == "kinogaki_phong_surface")
    );
    assert!(!back[0].double_sided);
}

#[test]
fn includes_read_the_documents_they_name_relative_to_the_one_naming_them() {
    let files: HashMap<&str, &str> = HashMap::from([
        (
            "lib/defs.mtlx",
            "<materialx version=\"1.39\" fileprefix=\"maps/\">\n  <xi:include href=\"more.mtlx\" />\n  <xi:include href=\"./more.mtlx\" />\n  <constant name=\"half\" type=\"float\">\n    <input name=\"value\" type=\"float\" value=\"0.5\" />\n  </constant>\n</materialx>\n",
        ),
        (
            "lib/more.mtlx",
            "<materialx version=\"1.39\">\n  <xi:include href=\"../lib/./defs.mtlx\" />\n  <constant name=\"more\" type=\"float\" />\n</materialx>\n",
        ),
    ]);
    // Each name resolved against the folder of the document naming it, the
    // file's identity its path without `.` and `..`, as a resolver gives it.
    let identity = |href: &str, within: Option<&str>| {
        let mut parts: Vec<&str> = within
            .map_or(vec![], |w| w.split('/').collect::<Vec<_>>())
            .into_iter()
            .rev()
            .skip(1)
            .rev()
            .collect();
        for part in href.split('/') {
            match part {
                "." => {}
                ".." => _ = parts.pop(),
                part => parts.push(part),
            }
        }
        parts.join("/")
    };
    let text = "<materialx version=\"1.39\">\n  <xi:include href=\"lib/defs.mtlx\" />\n  <xi:include href=\"gone.mtlx\" />\n  <xi:include href=\"lib/more.mtlx\" />\n</materialx>\n";
    let mut document: Document = text.parse().unwrap();
    let mut asked = Vec::new();
    let said = document.include(None, &mut |href, within| {
        let path = identity(href, within);
        asked.push(path.clone());
        let text = files.get(&*path)?.to_string();
        Some(Included {
            identity: path,
            text,
        })
    });
    assert_eq!(
        asked,
        [
            "lib/defs.mtlx",
            "lib/more.mtlx",
            "lib/defs.mtlx",
            "lib/more.mtlx",
            "gone.mtlx",
            "lib/more.mtlx"
        ]
        .map(String::from)
    );
    let [cycle] = &said[..] else {
        panic!("a cycle, said; a missing file, its loader's to say: {said:?}")
    };
    assert!(
        cycle.starts_with("../lib/./defs.mtlx includes itself"),
        "{said:?}"
    );
    let written = document.to_string();
    assert!(
        written.contains("<constant name=\"half\" type=\"float\" fileprefix=\"maps/\">")
            && !written.contains("xi:include"),
        "{written}"
    );
    assert_eq!(
        written.matches("name=\"more\"").count(),
        1,
        "included once: {written}"
    );
}

#[test]
fn includes_naming_one_document_many_times_read_it_once() {
    // Each level includes the next twice: read whole, 2^30 copies.
    let level = |k: usize| {
        let next = format!("<xi:include href=\"{}.mtlx\" />", k + 1);
        format!(
            "<materialx version=\"1.39\">{next}{next}<constant name=\"c{k}\" type=\"float\" /></materialx>"
        )
    };
    let mut document: Document = level(0).parse().unwrap();
    let mut loads = 0;
    let said = document.include(Some("0.mtlx"), &mut |href, _| {
        loads += 1;
        let k: usize = href.trim_end_matches(".mtlx").parse().unwrap();
        let text = match k < 30 {
            true => level(k),
            false => "<materialx version=\"1.39\" />".to_owned(),
        };
        Some(Included {
            identity: href.to_owned(),
            text,
        })
    });
    assert_eq!(
        loads, 60,
        "each level's second include is asked, then left out"
    );
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(document.to_string().matches("<constant").count(), 30);
}

/// `document` flattened on a thread of the 2 MiB stack a file is read on.
fn flattened_on_a_small_stack(document: Document) -> (Vec<Material>, Vec<String>) {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || document.materials())
        .unwrap()
        .join()
        .unwrap()
}

/// A material whose surface's `base` reads node `read` of `nodes`.
fn reading(nodes: &str, read: &str) -> Document {
    format!(
        "<materialx version=\"1.39\">{nodes}<standard_surface name=\"s\" type=\"surfaceshader\"><input name=\"base\" type=\"float\" nodename=\"{read}\" /></standard_surface><surfacematerial name=\"m\" type=\"material\"><input name=\"surfaceshader\" type=\"surfaceshader\" nodename=\"s\" /></surfacematerial></materialx>"
    )
    .parse()
    .unwrap()
}

#[test]
fn nodes_reading_themselves_are_left_unbound_and_said() {
    let add = |name: &str, reads: &str| {
        format!(
            "<add name=\"{name}\" type=\"float\"><input name=\"in1\" type=\"float\" nodename=\"{reads}\" /></add>"
        )
    };
    for (nodes, said) in [
        (add("a", "a"), "a reads itself"),
        (add("a", "b") + &add("b", "a"), "a reads itself"),
    ] {
        let (materials, left) = flattened_on_a_small_stack(reading(&nodes, "a"));
        assert_eq!(materials.len(), 1);
        assert!(left.iter().any(|l| l.contains(said)), "{left:?}");
    }
}

#[test]
fn a_chain_of_nodes_deeper_than_its_limit_is_cut_and_said() {
    // 2,000 nodes, each reading the next: written deepest last, so the
    // material reads the whole chain from its top.
    let add = |k: usize| {
        format!(
            "<add name=\"n{k}\" type=\"float\"><input name=\"in1\" type=\"float\" nodename=\"n{}\" /></add>",
            k + 1
        )
    };
    let nodes: String =
        (0..2000).map(add).collect::<String>() + "<constant name=\"n2000\" type=\"float\" />";
    let (materials, left) = flattened_on_a_small_stack(reading(&nodes, "n0"));
    assert_eq!(materials.len(), 1);
    assert!(
        left.iter()
            .any(|l| l.contains("ends a chain of nodes deeper than 256")),
        "{left:?}"
    );
}

#[test]
fn an_input_s_unit_is_its_own_else_its_node_s_and_writes_back() {
    let text = r#"<?xml version="1.0"?>
<materialx version="1.39" unit="meter" unittype="distance">
  <gltf_pbr name="glass" type="surfaceshader" unit="centimeter" unittype="distance">
    <input name="thickness" type="float" value="2" />
    <input name="attenuation_distance" type="float" value="5" unit="millimeter" unittype="distance" />
  </gltf_pbr>
  <open_pbr_surface name="honey" type="surfaceshader">
    <input name="transmission_depth" type="float" value="1" />
  </open_pbr_surface>
  <surfacematerial name="Glass" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="glass" />
  </surfacematerial>
  <surfacematerial name="Honey" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="honey" />
  </surfacematerial>
</materialx>
"#;
    let document: Document = text.parse().unwrap();
    let (materials, said) = document.materials();
    assert!(said.is_empty(), "{said:?}");
    let units = |m: &Material, name: &str| -> Vec<(String, String)> {
        let units = &node(m, name).units;
        units.iter().map(|(i, u)| (i.clone(), u.clone())).collect()
    };
    let unit = |input: &str, unit: &str| (input.to_owned(), unit.to_owned());
    assert_eq!(
        units(&materials[0], "glass"),
        [
            unit("attenuation_distance", "millimeter"),
            unit("thickness", "centimeter")
        ]
    );
    // The document's own, where nothing nearer says.
    assert_eq!(
        units(&materials[1], "honey"),
        [unit("transmission_depth", "meter")]
    );
    let written = Document::of_materials(&materials, "lin_rec709").to_string();
    let said = r#"name="thickness" type="float" value="2" unittype="distance" unit="centimeter""#;
    assert!(written.contains(said), "{written}");
    let (again, _) = written.parse::<Document>().unwrap().materials();
    assert_eq!(units(&again[0], "glass"), units(&materials[0], "glass"));
}
