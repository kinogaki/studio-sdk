//! [`Graph`]: a definition's graph implementation, resolved and checked: each
//! node's definition found as MaterialX finds it, each input bound to a value,
//! another node's output or the definition's own input of the same type, and
//! each of the definition's outputs to what yields it. What expands a node
//! into its definition reads this, once per definition.
use crate::library::{Library, NodeDef};
use crate::types::{Type, Value};
use crate::xml::Element;
use std::collections::HashMap;
use std::sync::Arc;

/// What an input of a graph's node (or one of its outputs) is bound to.
#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    /// A value of the input's type.
    Value(Value),
    /// Output `output` of the graph's node `node` (by index).
    Output {
        /// The node, by its index in [`Graph::nodes`].
        node: usize,
        /// Its output, by name.
        output: String,
    },
    /// The definition's own input of this name: what a node of it holds there,
    /// or what is connected to it.
    Interface(String),
}

/// One node of a graph: its name, definition and bound inputs (an input not
/// listed holds its definition's default).
#[derive(Clone, Debug, PartialEq)]
pub struct GraphNode {
    /// Its name in the graph.
    pub name: String,
    /// Its definition.
    pub def: Arc<NodeDef>,
    /// Its inputs that the graph binds, by name.
    pub inputs: Vec<(String, Binding)>,
}

/// A definition's graph (see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    /// Its nodes; a binding names them by index.
    pub nodes: Vec<GraphNode>,
    /// What yields each of the definition's outputs, by name.
    pub outputs: Vec<(String, Binding)>,
}

/// `source`, the graph implementing `def`, resolved against `library`.
pub(crate) fn resolve(library: &Library, def: &NodeDef, source: &Element) -> Result<Graph, String> {
    let at = |what: &str| format!("{}: {what}", source.get("name"));
    let elements: Vec<&Element> = (source.elements())
        .filter(|e| !matches!(&*e.category, "input" | "output"))
        .collect();
    let index: HashMap<&str, usize> = (elements.iter().enumerate())
        .map(|(k, e)| (e.get("name"), k))
        .collect();
    // Each node's definition first: a binding checks its source's type.
    let mut defs = Vec::with_capacity(elements.len());
    for e in &elements {
        defs.push(node_def(library, e).map_err(|why| at(&why))?.clone());
    }
    let bind = |e: &Element, ty: &Type| -> Result<Binding, String> {
        if let Some(interface) = e.attr("interfacename") {
            let port = def.input(interface).ok_or_else(|| {
                format!(
                    "{} reads interface {interface}, which isn't an input",
                    e.get("name")
                )
            })?;
            return match port.ty == *ty {
                true => Ok(Binding::Interface(interface.to_owned())),
                false => Err(format!("{interface} is a {}, not a {ty}", port.ty)),
            };
        }
        if let Some(name) = e.attr("nodename") {
            let &node = (index.get(name)).ok_or_else(|| format!("no node {name}"))?;
            let source: &NodeDef = &defs[node];
            let output = match e.attr("output") {
                Some(output) => source.output(output),
                None => source.outputs.first(),
            };
            let output = output.ok_or_else(|| format!("{name} has no such output"))?;
            return match output.ty == *ty {
                true => Ok(Binding::Output {
                    node,
                    output: output.name.clone(),
                }),
                false => Err(format!(
                    "{name}'s {} is a {}, not a {ty}",
                    output.name, output.ty
                )),
            };
        }
        let text = e
            .attr("value")
            .ok_or_else(|| format!("{} is bound to nothing", e.get("name")))?;
        let value = Value::parse(ty, text).ok_or_else(|| format!("{text:?} isn't a {ty}"))?;
        Ok(Binding::Value(value))
    };
    let mut nodes = Vec::with_capacity(elements.len());
    for (e, node_def) in elements.iter().zip(&defs) {
        let mut inputs = Vec::new();
        for input in e.elements().filter(|c| c.category == "input") {
            let name = input.get("name");
            let port = (node_def.input(name))
                .ok_or_else(|| at(&format!("{} has no input {name}", node_def.name)))?;
            let binding = bind(input, &port.ty).map_err(|why| at(&why))?;
            inputs.push((name.to_owned(), binding));
        }
        nodes.push(GraphNode {
            name: e.get("name").to_owned(),
            def: node_def.clone(),
            inputs,
        });
    }
    let mut outputs = Vec::new();
    for port in &def.outputs {
        let output = (source.elements())
            .find(|e| e.category == "output" && e.get("name") == port.name)
            .ok_or_else(|| at(&format!("nothing yields output {}", port.name)))?;
        outputs.push((
            port.name.clone(),
            bind(output, &port.ty).map_err(|why| at(&why))?,
        ));
    }
    Ok(Graph { nodes, outputs })
}

/// The definition node element `e` resolves to: the one it names, else the
/// one of its category, type and inputs' types.
pub(crate) fn node_def<'a>(library: &'a Library, e: &Element) -> Result<&'a Arc<NodeDef>, String> {
    if let Some(name) = e.attr("nodedef") {
        return library
            .def(name)
            .ok_or_else(|| format!("no definition {name}"));
    }
    let inputs: Vec<(&str, &str)> = (e.elements())
        .filter(|c| c.category == "input")
        .map(|c| (c.get("name"), c.get("type")))
        .collect();
    let version = e.attr("version");
    library
        .resolve(&e.category, e.get("type"), &inputs, version)
        .ok_or_else(|| {
            let name = e.get("name");
            format!(
                "{name}: no definition of {} {} takes its inputs",
                e.get("type"),
                e.category
            )
        })
}
