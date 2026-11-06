//! The upgrade from 1.38 to 1.39: `channels` attributes and `swizzle` nodes as
//! `constant`, `extract`, `convert`, `combine` and `separate` nodes;
//! thin-film layers as inputs of the BSDFs below them; subsurface radii as
//! colors; switches of ten inputs; `atan2`'s inputs renamed; normal maps
//! without their space.
use super::tree::{Id, Tree};

/// A channel letter's index.
fn index(channel: char) -> Option<usize> {
    match channel {
        'r' | 'x' => Some(0),
        'g' | 'y' => Some(1),
        'b' | 'z' => Some(2),
        'a' | 'w' => Some(3),
        _ => None,
    }
}

/// A constant channel's value (`0`, `1`).
fn constant(channel: char) -> Option<&'static str> {
    match channel {
        '0' => Some("0"),
        '1' => Some("1"),
        _ => None,
    }
}

/// The channels of a type a swizzle reads or makes.
fn count(ty: &str) -> Option<usize> {
    match ty {
        "float" => Some(1),
        "vector2" => Some(2),
        "color3" | "vector3" => Some(3),
        "color4" | "vector4" => Some(4),
        _ => None,
    }
}

/// The swizzles that are conversions: their channels and source width.
const CONVERSIONS: [(&str, usize); 10] = [
    ("rgb", 3),
    ("rgb", 4),
    ("rgba", 4),
    ("xyz", 3),
    ("xyz", 4),
    ("xyzw", 4),
    ("rr", 1),
    ("rrr", 1),
    ("xx", 1),
    ("xxx", 1),
];

pub(super) fn from_138(t: &mut Tree) {
    channels_as_swizzles(t);
    let mut unused = Vec::new();
    for node in t.nodes() {
        match t.category(node) {
            "layer" => thin_film(t, node, &mut unused),
            "subsurface_bsdf" => {
                let Some(radius) =
                    input(t, node, "radius").filter(|&r| t.get(r, "type") == "vector3")
                else {
                    continue;
                };
                let Some(graph) = t.parent(node) else {
                    continue;
                };
                let name = t.free_name(graph, "convert");
                let convert = t.add(
                    graph,
                    None,
                    "convert",
                    &[("name", &name), ("type", "color3")],
                );
                t.copy_input(node, "radius", convert, "in");
                for gone in ["value", "nodename", "nodegraph", "output", "interfacename"] {
                    t.unset(radius, gone);
                }
                t.set(radius, "nodename", &name);
                t.set(radius, "type", "color3");
            }
            "switch" => {
                if let Some(which) = input(t, node, "which")
                    && let Some(value) = t.attr(which, "value")
                {
                    let ty = t.get(which, "type");
                    let past = value.trim().parse::<f64>().is_ok_and(|v| v >= 5.0);
                    if past {
                        t.set(which, "value", if ty == "integer" { "0" } else { "0.0" });
                    }
                }
            }
            "swizzle" => swizzle(t, node),
            "atan2" => {
                if let Some(a) = input(t, node, "in1") {
                    t.set(a, "name", "iny");
                }
                if let Some(b) = input(t, node, "in2") {
                    t.set(b, "name", "inx");
                }
            }
            "normalmap" => normal_map(t, node),
            _ => {}
        }
    }
    unused.into_iter().for_each(|n| t.remove(n));
}

/// Node `node`'s input `name`.
fn input(t: &Tree, node: Id, name: &str) -> Option<Id> {
    t.of(node, "input").into_iter().find(|&i| t.name(i) == name)
}

/// Every port's `channels` as a `swizzle` node between it and what it
/// reads, made into modern nodes by the second pass.
fn channels_as_swizzles(t: &mut Tree) {
    for port in t.walk() {
        if !matches!(t.category(port), "input" | "output") {
            continue;
        }
        let channels = t.get(port, "channels").to_owned();
        if channels.is_empty() {
            continue;
        }
        let Some(graph) = t.graph_of(port) else {
            continue;
        };
        let upstream = t.connected(port).map(|n| t.get(n, "type").to_owned());
        let mut source = upstream.unwrap_or_default();
        if source.is_empty() || source == "multioutput" {
            source = match channels.as_str() {
                "xx" | "xxx" | "xxxx" => "float",
                "xyz" | "x" | "y" | "z" => "vector3",
                "rgba" | "a" => "color4",
                _ if t.get(port, "type") == "color3" => "color4",
                _ => "color3",
            }
            .to_owned();
        }
        let ty = t.get(port, "type").to_owned();
        if source == "float" && ty == "float" {
            t.unset(port, "channels");
            continue;
        }
        let parent = t.parent(port).expect("a port has a parent");
        let at = match t.parent(parent) == Some(graph) {
            true => t.index(parent),
            false => t.index(port),
        };
        let name = t.free_name(graph, "swizzle");
        let node = t.add(graph, at, "swizzle", &[("name", &name), ("type", &ty)]);
        let into = t.add(node, None, "input", &[("name", "in")]);
        t.copy_attributes(port, into);
        t.set(into, "name", "in");
        t.unset(into, "channels");
        t.set(into, "type", &source);
        let channels_input = t.add(
            node,
            None,
            "input",
            &[("name", "channels"), ("type", "string")],
        );
        t.set(channels_input, "value", &channels);
        for gone in ["value", "nodegraph", "output", "interfacename", "channels"] {
            t.unset(port, gone);
        }
        t.set(port, "nodename", &name);
    }
}

/// A `layer` of a `thin_film_bsdf` over a base: the film's thickness and
/// index on every BSDF upstream of it that takes them, the layer bypassed.
fn thin_film(t: &mut Tree, layer: Id, unused: &mut Vec<Id>) {
    let (Some(top), Some(base)) = (
        input(t, layer, "top").and_then(|i| t.connected(i)),
        input(t, layer, "base").and_then(|i| t.connected(i)),
    ) else {
        return;
    };
    if t.category(top) != "thin_film_bsdf" {
        return;
    }
    for upstream in upstream_nodes(t, layer) {
        let takes = matches!(
            t.category(upstream),
            "dielectric_bsdf" | "conductor_bsdf" | "generalized_schlick_bsdf"
        );
        let transmits =
            input(t, upstream, "scatter_mode").is_some_and(|m| t.get(m, "value") == "T");
        if takes && !transmits {
            t.copy_input(top, "thickness", upstream, "thinfilm_thickness");
            t.copy_input(top, "ior", upstream, "thinfilm_ior");
        }
    }
    let base_name = t.name(base).to_owned();
    for port in t.downstream(layer) {
        t.set(port, "nodename", &base_name);
    }
    unused.extend([layer, top]);
}

/// The nodes upstream of `node`, nearest first, each once.
fn upstream_nodes(t: &Tree, node: Id) -> Vec<Id> {
    let mut seen = Vec::new();
    let mut open = vec![node];
    while let Some(at) = open.pop() {
        for i in t.of(at, "input") {
            if let Some(up) = t.connected(i)
                && !seen.contains(&up)
            {
                seen.push(up);
                open.push(up);
            }
        }
    }
    seen
}

/// A `swizzle` as a `constant`, `extract`, `convert`, `combine`, or
/// `separate` and `combine`, by what it reads and makes.
fn swizzle(t: &mut Tree, node: Id) {
    let into = input(t, node, "in");
    let source = into.map_or_else(|| "float".to_owned(), |i| t.get(i, "type").to_owned());
    let ty = t.get(node, "type").to_owned();
    let (Some(from), Some(to)) = (count(&source), count(&ty)) else {
        return;
    };
    let channels_input = input(t, node, "channels");
    let channels: Vec<char> =
        channels_input.map_or_else(Vec::new, |c| t.get(c, "value").chars().collect());
    if let Some(i) = into
        && t.attr(i, "value").is_some()
        && ["nodename", "nodegraph", "interfacename"]
            .iter()
            .any(|k| t.attr(i, k).is_some())
    {
        t.unset(i, "value");
    }
    let defined = t.attr(node, "nodedef").is_some();
    let all_constant = (0..to).all(|k| channels.get(k).copied().and_then(constant).is_some());
    let is_value = into.is_none_or(|i| t.attr(i, "value").is_some());
    if is_value || all_constant {
        t.set_category(node, "constant");
        if defined {
            t.set(node, "nodedef", &format!("ND_constant_{ty}"));
        }
        let text = into.map_or_else(|| "0".to_owned(), |i| t.get(i, "value").to_owned());
        let tokens: Vec<&str> = text.split(',').map(str::trim).collect();
        let values: Vec<String> = (0..to)
            .map(|k| {
                let ch = channels.get(k).copied();
                if let Some(at) = ch.and_then(index)
                    && let Some(token) = tokens.get(at)
                {
                    return (*token).to_owned();
                }
                match ch.and_then(constant) {
                    // MaterialX writes these through std::to_string.
                    Some(c) => format!("{c}.000000"),
                    None => tokens[0].to_owned(),
                }
            })
            .collect();
        let value = t.add(node, None, "input", &[("name", "value"), ("type", &ty)]);
        t.set(value, "value", &values.join(", "));
        if let Some(i) = into {
            t.remove(i);
        }
    } else if to == 1 {
        t.set_category(node, "extract");
        if defined {
            t.set(node, "nodedef", &format!("ND_extract_{source}"));
        }
        if let Some(at) = channels.first().copied().and_then(index) {
            let index = t.input(node, "index", "integer");
            t.set(index, "value", &at.to_string());
        }
    } else if source != ty
        && CONVERSIONS.contains(&(channels.iter().collect::<String>().as_str(), from))
    {
        t.set_category(node, "convert");
        if defined {
            t.set(node, "nodedef", &format!("ND_convert_{source}_{ty}"));
        }
    } else if from == 1 {
        let into = into.expect("a swizzle of a connection has its input");
        let into_name = t.name(into).to_owned();
        t.set_category(node, &format!("combine{to}"));
        if defined {
            t.set(node, "nodedef", &format!("ND_combine{to}_{ty}"));
        }
        for k in 0..to {
            let name = format!("in{}", k + 1);
            match channels.get(k).copied().and_then(constant) {
                Some(c) => {
                    let part = t.add(node, None, "input", &[("name", &name), ("type", "float")]);
                    t.set(part, "value", c);
                }
                None => {
                    t.add(node, None, "input", &[("name", &name), ("type", "float")]);
                    t.copy_input(node, &into_name, node, &name);
                }
            }
        }
        t.remove(into);
    } else {
        let into = into.expect("a swizzle of a connection has its input");
        let into_name = t.name(into).to_owned();
        let color = t.get(into, "type").starts_with("color");
        let graph = t.parent(node).expect("a node is in a graph");
        let separate = t.free_name(graph, "separate");
        let at = t.index(node);
        let split = t.add(
            graph,
            at,
            &format!("separate{from}"),
            &[("name", &separate), ("type", "multioutput")],
        );
        t.set_category(node, &format!("combine{to}"));
        if defined {
            t.set(node, "nodedef", &format!("ND_combine{to}_{ty}"));
        }
        for k in 0..to {
            let name = format!("in{}", k + 1);
            let part = t.add(node, None, "input", &[("name", &name), ("type", "float")]);
            let ch = channels.get(k).copied();
            match (ch.filter(|c| index(*c).is_some()), ch.and_then(constant)) {
                (Some(c), _) => {
                    t.set(part, "nodename", &separate);
                    t.set(part, "output", &format!("out{c}"));
                }
                (None, Some(c)) => t.set(part, "value", c),
                (None, None) => {
                    t.set(part, "nodename", &separate);
                    t.set(part, "output", if color { "outr" } else { "outx" });
                }
            }
        }
        t.copy_input(node, &into_name, split, "in");
        t.remove(into);
    }
    if let Some(c) = channels_input {
        t.remove(c);
    }
}

/// A normal map: one in object space as the graph it means, else without its
/// space input, its bitangent made from its normal and tangent when it has
/// those alone.
fn normal_map(t: &mut Tree, node: Id) {
    let graph = t.parent(node).expect("a node is in a graph");
    let space = input(t, node, "space");
    if space.is_some_and(|s| t.get(s, "value") == "object") {
        let multiply = t.free_name(graph, "multiply");
        let m = t.add(
            graph,
            None,
            "multiply",
            &[("name", &multiply), ("type", "vector3")],
        );
        t.copy_input(node, "in", m, "in1");
        let two = t.add(m, None, "input", &[("name", "in2"), ("type", "float")]);
        t.set(two, "value", "2");
        let subtract = t.free_name(graph, "subtract");
        let s = t.add(
            graph,
            None,
            "subtract",
            &[("name", &subtract), ("type", "vector3")],
        );
        let from = t.add(s, None, "input", &[("name", "in1"), ("type", "vector3")]);
        t.set(from, "nodename", &multiply);
        let one = t.add(s, None, "input", &[("name", "in2"), ("type", "float")]);
        t.set(one, "value", "1");
        t.set_category(node, "normalize");
        for i in t.of(node, "input") {
            t.remove(i);
        }
        let i = t.add(node, None, "input", &[("name", "in"), ("type", "vector3")]);
        t.set(i, "nodename", &subtract);
        if t.attr(node, "nodedef").is_some() {
            t.set(node, "nodedef", "ND_normalize_vector3");
        }
        return;
    }
    if let Some(space) = space {
        t.remove(space);
    }
    let (normal, tangent) = (input(t, node, "normal"), input(t, node, "tangent"));
    if (normal.is_some() || tangent.is_some()) && input(t, node, "bitangent").is_none() {
        let cross = t.free_name(graph, "normalmap_cross");
        let c = t.add(
            graph,
            None,
            "crossproduct",
            &[("name", &cross), ("type", "vector3")],
        );
        t.copy_input(node, "normal", c, "in1");
        t.copy_input(node, "tangent", c, "in2");
        let norm = t.free_name(graph, "normalmap_cross_norm");
        let n = t.add(
            graph,
            None,
            "normalize",
            &[("name", &norm), ("type", "vector3")],
        );
        let i = t.add(n, None, "input", &[("name", "in"), ("type", "vector3")]);
        t.set(i, "nodename", &cross);
        let b = t.add(
            node,
            None,
            "input",
            &[("name", "bitangent"), ("type", "vector3")],
        );
        t.set(b, "nodename", &norm);
    }
    if t.get(node, "nodedef") == "ND_normalmap" {
        t.set(node, "nodedef", "ND_normalmap_float");
    }
}
