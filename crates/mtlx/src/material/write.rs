//! A document made of materials (the module docs say how).
use super::{Bound, Material, MaterialNode, unique, valid_name};
use crate::document::{Document, VERSION};
use crate::library::{NodeDef, library};
use crate::types::Type;
use crate::xml::{self, Child, Element};
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::OnceLock;

/// Kinogaki's definitions as the library reads them (`kinogaki/`): what a
/// document using one carries, so any MaterialX reader resolves it.
const KINOGAKI: [&str; 2] = [
    include_str!("../../kinogaki/kinogaki_defs.mtlx"),
    include_str!("../../kinogaki/kinogaki_ng.mtlx"),
];

impl Document {
    /// A document of `materials` (see the module docs), its values' colors
    /// in `colorspace` (`lin_rec709`), and Kinogaki's definitions those use.
    pub fn of_materials(materials: &[Material], colorspace: &str) -> Document {
        let mut children = Vec::new();
        let used: Vec<&str> = (materials.iter())
            .flat_map(|m| m.nodes.iter().map(|n| n.def.name.as_str()))
            .collect();
        for definition in kinogaki(&used) {
            children.push(Child::Element(definition));
        }
        let mut taken = HashSet::new();
        for m in materials {
            if !children.is_empty() {
                children.push(Child::Newline);
            }
            let name = unique(valid_name(&m.name), &mut taken);
            let graph = unique(format!("NG_{name}"), &mut taken);
            let (graph, material) = material(m, &name, &graph, colorspace);
            children.extend(graph.map(Child::Element));
            children.push(Child::Element(material));
        }
        let root = Element {
            category: "materialx".into(),
            attributes: vec![
                ("version".into(), VERSION.into()),
                ("colorspace".into(), Cow::Owned(colorspace.to_owned())),
            ],
            children,
        };
        Document { root }
    }
}

/// `m` as a node graph of its nodes, when it has any besides its material
/// node, and its material node named `name`, reading graph `graph`.
fn material(m: &Material, name: &str, graph: &str, colorspace: &str) -> (Option<Element>, Element) {
    let mut taken = HashSet::new();
    let names: Vec<String> = (m.nodes.iter())
        .map(|n| unique(valid_name(&n.name), &mut taken))
        .collect();
    let mut held = Vec::new();
    let written = Named {
        nodes: &m.nodes,
        names: &names,
        colorspace,
    };
    for (at, node) in m
        .nodes
        .iter()
        .enumerate()
        .filter(|(at, _)| *at != m.material)
    {
        held.push(Child::Element(written.element(node, &names[at])));
    }
    let shown = &m.nodes[m.material];
    let mut outputs = Vec::new();
    let mut inputs = Vec::new();
    for (input, bound) in &shown.inputs {
        let ty = port_type(&shown.def, input);
        match bound {
            Bound::Output { node, output } => {
                let mut out = Element {
                    category: "output".into(),
                    ..Element::default()
                };
                out.set("name", input);
                out.set("type", ty.name());
                out.set("nodename", &names[*node]);
                if m.nodes[*node].def.outputs.len() > 1 {
                    out.set("output", output);
                }
                outputs.push(Child::Element(out));
                let mut read = Element {
                    category: "input".into(),
                    ..Element::default()
                };
                read.set("name", input);
                read.set("type", ty.name());
                read.set("nodegraph", graph);
                read.set("output", input);
                inputs.push(Child::Element(read));
            }
            Bound::Value { .. } => inputs.push(Child::Element(written.input(input, &ty, bound))),
        }
    }
    let mut material = header(shown, name);
    if name != m.name || m.label.is_some() {
        material.set("uiname", m.label.as_deref().unwrap_or(&m.name));
    }
    if !m.double_sided {
        material.set("kinogaki:doublesided", "false");
    }
    material.children = inputs;
    let graph = (!held.is_empty()).then(|| {
        let mut element = Element {
            category: "nodegraph".into(),
            ..Element::default()
        };
        element.set("name", graph);
        held.extend(outputs);
        element.children = held;
        element
    });
    (graph, material)
}

/// A material's nodes as they are written: by the names they take.
struct Named<'a> {
    nodes: &'a [MaterialNode],
    names: &'a [String],
    /// The document's color space.
    colorspace: &'a str,
}

impl Named<'_> {
    /// Node `node` named `name` as an element: its category, type, the
    /// definition it is when nothing else would say so, and its inputs.
    fn element(&self, node: &MaterialNode, name: &str) -> Element {
        let mut e = header(node, name);
        for (input, bound) in &node.inputs {
            let ty = port_type(&node.def, input);
            e.children
                .push(Child::Element(self.input(input, &ty, bound)));
        }
        e
    }

    /// Input `input` of type `ty` bound to `bound`.
    fn input(&self, input: &str, ty: &Type, bound: &Bound) -> Element {
        let mut e = Element {
            category: "input".into(),
            ..Element::default()
        };
        e.set("name", input);
        e.set("type", ty.name());
        match bound {
            Bound::Value { value, colorspace } => {
                e.set("value", &value.to_string());
                // A value of no color space says so, or it would be read in
                // the document's.
                let said = match colorspace.as_deref() {
                    Some(space) if space == self.colorspace => None,
                    Some(space) => Some(space),
                    None => Some(""),
                };
                if let Some(space) = said.filter(|_| ty.is_color() || *ty == Type::Filename) {
                    e.set("colorspace", space);
                }
            }
            Bound::Output { node, output } => {
                e.set("nodename", &self.names[*node]);
                // A node of one output is read by its name alone.
                if self.nodes[*node].def.outputs.len() > 1 {
                    e.set("output", output);
                }
            }
        }
        e
    }
}

/// `node`'s element named `name`, holding no inputs: its category, type
/// and, where its category, type and inputs' types would resolve to another
/// definition, its `nodedef`.
fn header(node: &MaterialNode, name: &str) -> Element {
    let def = &node.def;
    let mut e = Element {
        category: Cow::Owned(def.node.clone()),
        ..Element::default()
    };
    e.set("name", name);
    e.set("type", def.ty().name());
    let typed: Vec<(&str, String)> = (node.inputs.iter())
        .map(|(input, _)| (input.as_str(), port_type(def, input).name().to_owned()))
        .collect();
    let typed: Vec<(&str, &str)> = typed.iter().map(|(n, t)| (*n, t.as_str())).collect();
    let resolved = library().resolve(&def.node, def.ty().name(), &typed, None);
    if resolved.is_none_or(|r| r.name != def.name) {
        e.set("nodedef", &def.name);
    }
    e
}

/// The type of `def`'s input `input`.
fn port_type(def: &NodeDef, input: &str) -> Type {
    def.input(input)
        .map_or_else(|| Type::of("float"), |p| p.ty.clone())
}

/// The elements of Kinogaki's definitions among `used` (by signature), and
/// their implementations, in the order the library holds them.
fn kinogaki(used: &[&str]) -> Vec<Element> {
    static ELEMENTS: OnceLock<Vec<Element>> = OnceLock::new();
    let all = ELEMENTS.get_or_init(|| {
        (KINOGAKI.iter())
            .filter_map(|text| xml::parse_static(text).ok())
            .flat_map(|root| root.children)
            .filter_map(|c| match c {
                Child::Element(e) => Some(e),
                _ => None,
            })
            .collect()
    });
    let wanted = |e: &Element| match &*e.category {
        "nodedef" => used.contains(&e.get("name")),
        "nodegraph" | "implementation" => used.contains(&e.get("nodedef")),
        _ => false,
    };
    all.iter().filter(|e| wanted(e)).cloned().collect()
}
