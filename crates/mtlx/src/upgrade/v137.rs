//! The upgrades from 1.36 to 1.37 and from 1.37 to 1.38.
use super::tree::{Id, Tree};

/// 1.36 to 1.37: definitions' types as outputs, legacy shader definitions
/// gone, geometric attributes as properties, renamed and split nodes,
/// backdrop nodes as backdrop elements.
pub(super) fn from_136(t: &mut Tree) {
    for def in t.of(0, "nodedef") {
        let ty = t.get(def, "type").to_owned();
        if t.attr(def, "type").is_none() {
            continue;
        }
        let mut output = None;
        if !ty.is_empty() && ty != "multioutput" {
            output = Some(
                match t
                    .of(def, "output")
                    .into_iter()
                    .find(|&o| t.name(o) == "out")
                {
                    Some(o) => o,
                    None => t.add(def, None, "output", &[("name", "out"), ("type", &ty)]),
                },
            );
        }
        t.unset(def, "type");
        let default = t.get(def, "defaultinput").to_owned();
        if let Some(output) = output.filter(|_| !default.is_empty()) {
            t.set(output, "defaultinput", &default);
        }
        t.unset(def, "defaultinput");
    }
    for def in t.of(0, "nodedef") {
        if t.attr(def, "shadertype").is_none() {
            continue;
        }
        let name = t.name(def).to_owned();
        for material in t.of(0, "material") {
            for shader in t.of(material, "shaderref") {
                if t.get(shader, "nodedef") == name {
                    t.unset(shader, "nodedef");
                }
            }
        }
        t.remove(def);
    }
    for info in t.of(0, "geominfo") {
        for attr in t.of(info, "geomattr") {
            t.set_category(attr, "geomprop");
        }
    }
    for node in t.nodes() {
        if t.category(node) == "geomattrvalue" {
            t.set_category(node, "geompropvalue");
            if let Some(name) = t.attr(node, "attrname").map(str::to_owned) {
                t.set(node, "geomprop", &name);
                t.unset(node, "attrname");
            }
        }
    }
    let mut unused = Vec::new();
    for node in t.nodes() {
        let ty = t.get(node, "type").to_owned();
        let input =
            |t: &Tree, name: &str| t.of(node, "input").into_iter().find(|&i| t.name(i) == name);
        match t.category(node) {
            "invert" if ty == "matrix33" || ty == "matrix44" => {
                t.set_category(node, "invertmatrix")
            }
            "rotate" => t.set_category(
                node,
                if ty == "vector2" {
                    "rotate2d"
                } else {
                    "rotate3d"
                },
            ),
            "compare" => {
                t.set_category(node, "ifgreatereq");
                if let Some(intest) = input(t, "intest") {
                    t.set(intest, "name", "value1");
                }
                if let Some(cutoff) = t.child(node, "cutoff") {
                    t.set_category(cutoff, "input");
                    t.set(cutoff, "name", "value2");
                }
                if let (Some(a), Some(b)) = (input(t, "in1"), input(t, "in2")) {
                    t.set(a, "name", "in2");
                    t.set(b, "name", "in1");
                }
            }
            "transformpoint" | "transformvector" | "transformnormal" => {
                if t.child(node, "fromspace").is_none() && t.child(node, "tospace").is_none() {
                    t.set_category(node, "transformmatrix");
                }
            }
            "combine" => {
                let n = if t.child(node, "in4").is_some() {
                    "combine4"
                } else if t.child(node, "in3").is_some() {
                    "combine3"
                } else {
                    "combine2"
                };
                t.set_category(node, n);
            }
            "separate" => {
                if let Some(input) = input(t, "in") {
                    let n = match t.get(input, "type") {
                        "vector4" | "color4" => "separate4",
                        "vector3" | "color3" => "separate3",
                        _ => "separate2",
                    };
                    t.set_category(node, n);
                }
            }
            "backdrop" => {
                let name = t.name(node).to_owned();
                let backdrop = t.add(0, None, "backdrop", &[("name", &name)]);
                for parameter in t.of(node, "parameter") {
                    if let Some(value) = t.attr(parameter, "value").map(str::to_owned) {
                        let key = t.name(parameter).to_owned();
                        t.set(backdrop, &key, &value);
                    }
                }
                unused.push(node);
            }
            _ => {}
        }
    }
    unused.into_iter().for_each(|n| t.remove(n));
}

/// The BSDF nodes 1.38 renamed: each old name and its new one.
const BSDFS: [(&str, &str); 10] = [
    ("dielectric_brdf", "dielectric_bsdf"),
    ("dielectric_btdf", "dielectric_bsdf"),
    ("generalized_schlick_brdf", "generalized_schlick_bsdf"),
    ("conductor_brdf", "conductor_bsdf"),
    ("sheen_brdf", "sheen_bsdf"),
    ("diffuse_brdf", "oren_nayar_diffuse_bsdf"),
    ("burley_diffuse_brdf", "burley_diffuse_bsdf"),
    ("diffuse_btdf", "translucent_bsdf"),
    ("subsurface_brdf", "subsurface_bsdf"),
    ("thin_film_brdf", "thin_film_bsdf"),
];

/// 1.37 to 1.38: `color2` as `vector2`, material elements as material nodes,
/// BSDFs renamed and layered by `layer`, `artistic_ior` for conductors,
/// interface names apart from node names, parameters as inputs.
pub(super) fn from_137(t: &mut Tree) {
    color2_as_vector2(t);
    materials_as_nodes(t);
    let mut ior = Vec::new();
    for node in t.nodes() {
        let category = t.category(node).to_owned();
        let renamed = BSDFS
            .iter()
            .find(|(old, _)| *old == category)
            .map(|(_, new)| *new);
        match (category.as_str(), renamed) {
            ("atan2", _) => {
                let (a, b) = (named(t, node, "in1"), named(t, node, "in2"));
                if let Some(a) = a {
                    t.set(a, "name", "in2");
                }
                if let Some(b) = b {
                    t.set(b, "name", "in1");
                }
            }
            ("rotate3d", _) => {
                if let Some(axis) = t.child(node, "axis") {
                    t.set_category(axis, "input");
                }
            }
            ("dielectric_btdf", Some(new)) => {
                t.set_category(node, new);
                if let Some(interior) = named(t, node, "interior") {
                    t.remove(interior);
                }
                let mode = t.input(node, "scatter_mode", "string");
                t.set(mode, "value", "T");
            }
            ("conductor_brdf", Some(new)) => {
                t.set_category(node, new);
                conductor_ior(t, node);
            }
            (
                "dielectric_brdf" | "generalized_schlick_brdf" | "sheen_brdf" | "thin_film_brdf",
                Some(new),
            ) => {
                t.set_category(node, new);
                layering(t, node);
            }
            (_, Some(new)) => t.set_category(node, new),
            ("artistic_ior", _) => {
                for output in t.of(node, "output") {
                    if matches!(t.name(output), "ior" | "extinction") {
                        t.set(output, "type", "color3");
                    }
                }
            }
            _ => {}
        }
        for input in t.of(node, "input") {
            let output = t.get(input, "output");
            if matches!(output, "ior" | "extinction")
                && t.get(input, "type") == "vector3"
                && t.connected(input)
                    .is_some_and(|c| t.category(c) == "artistic_ior")
            {
                ior.push(input);
            }
        }
    }
    for input in ior {
        let (Some(source), Some(node)) = (t.connected(input), t.parent(input)) else {
            continue;
        };
        let Some(graph) = t.parent(node) else {
            continue;
        };
        let which = t.get(input, "output").to_owned();
        let name = format!("{}__convert_{which}", t.name(node));
        let source_name = t.name(source).to_owned();
        let convert = t.add(
            graph,
            None,
            "convert",
            &[("name", &name), ("type", "vector3")],
        );
        let at = t.add(
            convert,
            None,
            "input",
            &[("name", "in"), ("type", "color3")],
        );
        t.set(at, "nodename", &source_name);
        t.set(at, "output", &which);
        t.set(input, "nodename", &name);
        t.unset(input, "output");
    }
    for graph in t.of(0, "nodegraph") {
        t.unset(graph, "version");
        let mut interfaces = std::collections::BTreeSet::new();
        for node in t
            .elements(graph)
            .filter(|&n| t.is_node(n))
            .collect::<Vec<_>>()
        {
            for port in t.elements(node).collect::<Vec<_>>() {
                if let Some(name) = t.attr(port, "interfacename").filter(|n| !n.is_empty()) {
                    interfaces.insert(name.to_owned());
                }
            }
        }
        for interface in interfaces {
            let Some(node) = t.child(graph, &interface).filter(|&n| t.is_node(n)) else {
                continue;
            };
            let renamed = t.free_name(graph, &interface);
            for port in t.downstream(node) {
                if t.get(port, "nodename") == interface {
                    t.set(port, "nodename", &renamed);
                }
            }
            t.set(node, "name", &renamed);
        }
    }
    for e in t.walk() {
        let interface = t.is_node(e) || matches!(t.category(e), "nodedef" | "nodegraph");
        if !interface {
            continue;
        }
        for parameter in t.of(e, "parameter") {
            t.set_category(parameter, "input");
            if t.category(e) == "nodedef" {
                t.set(parameter, "uniform", "true");
            }
        }
    }
}

/// Node `node`'s input `name`.
fn named(t: &Tree, node: Id, name: &str) -> Option<Id> {
    t.of(node, "input").into_iter().find(|&i| t.name(i) == name)
}

/// `color2` types as `vector2`, the channels read of them renamed.
fn color2_as_vector2(t: &mut Tree) {
    let swap = |text: &str| text.replace('r', "x").replace('a', "y");
    for e in t.walk() {
        if t.get(e, "type") != "color2" {
            continue;
        }
        t.set(e, "type", "vector2");
        let Some(node) = t.parent(e).filter(|&p| t.is_node(p)) else {
            continue;
        };
        for port in t.downstream(node) {
            if let Some(channels) = t.attr(port, "channels").map(swap) {
                t.set(port, "channels", &channels);
            }
            if let Some(output) = t.attr(port, "output").filter(|o| !o.is_empty()).map(swap) {
                t.set(port, "output", &output);
            }
        }
        if let Some(channels) = t.child(node, "channels")
            && let Some(value) = t.attr(channels, "value").map(swap)
        {
            t.set(channels, "value", &value);
        }
    }
}

/// Material elements as `surfacematerial` nodes over shader nodes made of
/// their shader references.
fn materials_as_nodes(t: &mut Tree) {
    for material in t.of(0, "material") {
        let mut made: Option<Id> = None;
        for shader in t.of(material, "shaderref") {
            let category = t.get(shader, "node").to_owned();
            let name = t.free_name(0, t.name(shader));
            let node = t.add(
                0,
                None,
                &category,
                &[("name", &name), ("type", "surfaceshader")],
            );
            for key in ["nodedef", "target", "version"] {
                if let Some(value) = t
                    .attr(shader, key)
                    .filter(|v| !v.is_empty())
                    .map(str::to_owned)
                {
                    t.set(node, key, &value);
                }
            }
            for child in t.elements(shader).collect::<Vec<_>>() {
                let category = match t.category(child) {
                    "bindinput" | "bindparam" => "input",
                    "bindtoken" => "token",
                    _ => continue,
                };
                let added = t.add(node, None, category, &[]);
                t.copy_attributes(child, added);
            }
            let surface = match made {
                Some(m) => m,
                None => {
                    let name = t.free_name(0, "temp");
                    let m = t.add(
                        0,
                        None,
                        "surfacematerial",
                        &[("name", &name), ("type", "material")],
                    );
                    made = Some(m);
                    m
                }
            };
            if named(t, surface, "surfaceshader").is_none() {
                let input = t.add(
                    surface,
                    None,
                    "input",
                    &[("name", "surfaceshader"), ("type", "surfaceshader")],
                );
                t.set(input, "nodename", &name);
            }
        }
        t.remove(material);
        if let Some(made) = made {
            let name = t.name(material).to_owned();
            t.set(made, "name", &name);
            // The material's other attributes, where the node has none.
            let mut held = Tree::copy_of(t, material);
            held.retain(|(k, _)| t.attr(made, k).is_none());
            for (k, v) in held {
                t.set(made, &k, &v);
            }
        }
    }
}

/// A BSDF's `base` input as a `layer` node over it, taking its name.
fn layering(t: &mut Tree, node: Id) {
    let Some(base) = named(t, node, "base") else {
        return;
    };
    if let (Some(below), Some(graph)) = (t.connected(base), t.parent(node)) {
        let old = t.name(node).to_owned();
        t.set(node, "name", &format!("{old}__layer_top"));
        let below = t.name(below).to_owned();
        let layer = t.add(graph, None, "layer", &[("name", &old), ("type", "BSDF")]);
        let top = t.add(layer, None, "input", &[("name", "top"), ("type", "BSDF")]);
        t.set(top, "nodename", &format!("{old}__layer_top"));
        let under = t.add(layer, None, "input", &[("name", "base"), ("type", "BSDF")]);
        t.set(under, "nodename", &below);
    }
    t.remove(base);
}

/// A conductor's artistic inputs moved onto an `artistic_ior` node feeding its
/// physical ones.
fn conductor_ior(t: &mut Tree, node: Id) {
    let Some(graph) = t.parent(node) else {
        return;
    };
    let name = format!("{}__artistic_ior", t.name(node));
    let ior = t.add(
        graph,
        None,
        "artistic_ior",
        &[("name", &name), ("type", "multioutput")],
    );
    t.add(ior, None, "output", &[("name", "ior"), ("type", "color3")]);
    t.add(
        ior,
        None,
        "output",
        &[("name", "extinction"), ("type", "color3")],
    );
    t.copy_input(node, "reflectivity", ior, "reflectivity");
    t.copy_input(node, "edge_color", ior, "edge_color");
    for gone in ["reflectivity", "edge_color"] {
        if let Some(input) = named(t, node, gone) {
            t.remove(input);
        }
    }
    for output in ["ior", "extinction"] {
        let input = t.add(node, None, "input", &[("name", output), ("type", "color3")]);
        t.set(input, "nodename", &name);
        t.set(input, "output", output);
    }
}
