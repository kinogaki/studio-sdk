//! A document's materials as flat networks (MaterialX's own test documents,
//! `tests/documents/read`), a document written of materials reading back as
//! them, and includes read.
use kinogaki_mtlx::{Bound, Document, Material, Value};
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
            "<materialx version=\"1.39\" fileprefix=\"maps/\">\n  <xi:include href=\"more.mtlx\" />\n  <constant name=\"half\" type=\"float\">\n    <input name=\"value\" type=\"float\" value=\"0.5\" />\n  </constant>\n</materialx>\n",
        ),
        (
            "lib/more.mtlx",
            "<materialx version=\"1.39\">\n  <xi:include href=\"more.mtlx\" />\n</materialx>\n",
        ),
    ]);
    let text = "<materialx version=\"1.39\">\n  <xi:include href=\"lib/defs.mtlx\" />\n  <xi:include href=\"gone.mtlx\" />\n</materialx>\n";
    let mut document: Document = text.parse().unwrap();
    let mut asked = Vec::new();
    let said = document.include(&mut |path| {
        asked.push(path.to_owned());
        files
            .get(path)
            .map(|t| t.to_string())
            .ok_or_else(|| "no such file".to_owned())
    });
    assert_eq!(
        asked,
        ["lib/defs.mtlx", "lib/more.mtlx", "gone.mtlx"].map(String::from)
    );
    assert_eq!(said.len(), 2, "a cycle and a missing file: {said:?}");
    let written = document.to_string();
    assert!(
        written.contains("<constant name=\"half\" type=\"float\" fileprefix=\"maps/\">")
            && !written.contains("xi:include"),
        "{written}"
    );
}
