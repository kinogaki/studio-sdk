//! A document's materials, flattened (the module docs say how).
use super::{Bound, Material, MaterialNode, unique};
use crate::document::Document;
use crate::graph::node_def;
use crate::library::{NodeDef, Port, library};
use crate::types::{Type, Value};
use crate::xml::{DEPTH as CHAIN, Element};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// How deep definitions' graphs may hold one another.
const DEPTH: usize = 32;

impl Document {
    /// Each material node at the document's top level, with every node it
    /// reads, as a flat network ([`Material`]), in document order; and what
    /// was left out, said. A material that doesn't resolve is left out and
    /// said.
    pub fn materials(&self) -> (Vec<Material>, Vec<String>) {
        let doc = Doc::of(&self.root);
        let mut said = Vec::new();
        let looks = (self.root.elements()).filter(|e| matches!(&*e.category, "look" | "lookgroup"));
        if let Some(look) = looks.map(|e| e.get("name")).next() {
            said.push(format!(
                "looks ({look}…) and their material assignments are left out"
            ));
        }
        let mut materials = Vec::new();
        for e in self.root.elements().filter(|e| is_material(e)) {
            match Flat::new(&doc).material(e) {
                Ok((material, left)) => {
                    let name = &material.name;
                    said.extend(left.into_iter().map(|why| format!("{name}: {why}")));
                    materials.push(material);
                }
                Err(why) => said.push(format!("{}: {why}; left out", e.get("name"))),
            }
        }
        (materials, said)
    }
}

/// Whether `e` is a material node: of type `material`, or of a material's
/// category.
fn is_material(e: &Element) -> bool {
    e.get("type") == "material" || matches!(&*e.category, "surfacematerial" | "volumematerial")
}

/// What a document defines, by name.
struct Doc<'d> {
    root: &'d Element,
    /// Its top-level elements by name.
    named: HashMap<&'d str, &'d Element>,
    /// Its own node definitions (those the library lacks) that graphs
    /// implement: each node's, with its graph.
    defined: HashMap<&'d str, Vec<(Arc<NodeDef>, &'d Element)>>,
}

impl<'d> Doc<'d> {
    fn of(root: &'d Element) -> Doc<'d> {
        let named: HashMap<&str, &Element> = root.elements().map(|e| (e.get("name"), e)).collect();
        let mut graphs: HashMap<&str, &Element> = HashMap::new();
        for e in root.elements() {
            match &*e.category {
                "nodegraph" if e.attr("nodedef").is_some() => {
                    _ = graphs.insert(e.get("nodedef"), e)
                }
                "implementation" => {
                    if let Some(graph) = e.attr("nodegraph").and_then(|g| named.get(g)) {
                        graphs.insert(e.get("nodedef"), graph);
                    }
                }
                _ => {}
            }
        }
        let mut defined: HashMap<&str, Vec<(Arc<NodeDef>, &Element)>> = HashMap::new();
        let own = (root.elements())
            .filter(|e| e.category == "nodedef" && library().def(e.get("name")).is_none());
        for e in own {
            if let Some(graph) = graphs.get(e.get("name")) {
                (defined.entry(e.get("node")).or_default()).push((Arc::new(NodeDef::of(e)), graph));
            }
        }
        Doc {
            root,
            named,
            defined,
        }
    }

    /// The document's own definition node element `e` is of, with its
    /// graph: one of its category whose type and inputs fit.
    fn own(&self, e: &Element) -> Option<&(Arc<NodeDef>, &'d Element)> {
        let defs = self.defined.get(&*e.category)?;
        let inputs: Vec<(&str, &str)> = (e.elements().filter(|c| c.category == "input"))
            .map(|c| (c.get("name"), c.get("type")))
            .collect();
        defs.iter().find(|(def, _)| {
            let named = e.attr("nodedef").is_none_or(|n| n == def.name);
            let typed = def.ty().name() == e.get("type") || e.get("type").is_empty();
            let takes = (inputs.iter()).all(|(n, t)| {
                def.input(n)
                    .is_some_and(|p| t.is_empty() || p.ty.name() == *t)
            });
            named && typed && takes
        })
    }
}

/// Where a node is named: the document's top level, or a graph instance.
struct Scope<'d> {
    /// The graph whose nodes it names; none for the top level.
    graph: Option<&'d Element>,
    /// What each of its interface's inputs is bound to; none: the default.
    interface: HashMap<String, Option<Bound>>,
    /// Where it stands in the document, for the nodes' order.
    order: Vec<usize>,
    depth: usize,
}

/// What a node element made.
#[derive(Clone, Copy)]
enum Made {
    /// A node of the material, by index.
    Node(usize),
    /// A graph instance of a document's own definition, by scope.
    Instance(usize),
}

/// One material as it is flattened.
struct Flat<'a, 'd> {
    doc: &'a Doc<'d>,
    scopes: Vec<Scope<'d>>,
    /// Each node with where it stands in the document.
    nodes: Vec<(Vec<usize>, MaterialNode)>,
    made: HashMap<(usize, &'d str), Made>,
    /// The nodes being made, each reading the next: a node reading one of
    /// them reads itself, and the chain is at most [`CHAIN`] long, so
    /// making a node recurses within a small stack.
    making: Vec<(usize, &'d str)>,
    /// The top-level node graphs read, by name: their scopes.
    graphs: HashMap<&'d str, usize>,
    taken: HashSet<String>,
    left: Vec<String>,
}

impl<'a, 'd> Flat<'a, 'd> {
    fn new(doc: &'a Doc<'d>) -> Flat<'a, 'd> {
        let top = Scope {
            graph: None,
            interface: HashMap::new(),
            order: Vec::new(),
            depth: 0,
        };
        Flat {
            doc,
            scopes: vec![top],
            nodes: Vec::new(),
            made: HashMap::new(),
            making: Vec::new(),
            graphs: HashMap::new(),
            taken: HashSet::new(),
            left: Vec::new(),
        }
    }

    /// Material node `e` and every node it reads, ordered as the document
    /// holds them, and what was left out.
    fn material(mut self, e: &'d Element) -> Result<(Material, Vec<String>), String> {
        let Made::Node(material) = self.node(0, e.get("name"))? else {
            return Err("its definition is the document's own".into());
        };
        // Every node of a graph the material reads is the material's.
        let mut done: HashSet<&str> = HashSet::new();
        loop {
            let next = (self.graphs.iter())
                .filter(|(name, _)| !done.contains(*name))
                .map(|(name, scope)| (*name, *scope))
                .min();
            let Some((name, scope)) = next else { break };
            done.insert(name);
            let graph = self.scopes[scope].graph.expect("a graph's scope");
            for node in graph
                .elements()
                .filter(|c| !matches!(&*c.category, "input" | "output"))
            {
                if let Err(why) = self.node(scope, node.get("name")) {
                    self.left.push(format!("{why}, left out"));
                }
            }
        }
        let mut order: Vec<usize> = (0..self.nodes.len()).collect();
        order.sort_by(|&a, &b| self.nodes[a].0.cmp(&self.nodes[b].0));
        let mut at = vec![0; order.len()];
        for (new, &old) in order.iter().enumerate() {
            at[old] = new;
        }
        let mut nodes: Vec<Option<MaterialNode>> =
            self.nodes.into_iter().map(|(_, n)| Some(n)).collect();
        let mut sorted: Vec<MaterialNode> = order
            .iter()
            .map(|&old| nodes[old].take().expect("once"))
            .collect();
        for node in &mut sorted {
            for (_, bound) in &mut node.inputs {
                if let Bound::Output { node, .. } = bound {
                    *node = at[*node];
                }
            }
        }
        let material = Material {
            name: e.get("name").to_owned(),
            label: e.attr("uiname").map(str::to_owned),
            nodes: sorted,
            material: at[material],
            double_sided: e.attr("kinogaki:doublesided") != Some("false"),
        };
        Ok((material, self.left))
    }

    /// The element named `name` in `scope`.
    fn element(&self, scope: usize, name: &str) -> Option<&'d Element> {
        match self.scopes[scope].graph {
            Some(graph) => graph
                .elements()
                .find(|c| c.get("name") == name && c.category != "input"),
            None => self.doc.named.get(name).copied(),
        }
    }

    /// Node `name` of `scope`, made once; refused when it reads itself, or
    /// ends a chain of nodes reading one another deeper than [`CHAIN`].
    fn node(&mut self, scope: usize, name: &str) -> Result<Made, String> {
        let e = (self.element(scope, name)).ok_or_else(|| format!("no node {name}"))?;
        let key = (scope, e.get("name"));
        if let Some(made) = self.made.get(&key) {
            return Ok(*made);
        }
        if self.making.contains(&key) {
            return Err(format!("{name} reads itself"));
        }
        if self.making.len() >= CHAIN {
            return Err(format!("{name} ends a chain of nodes deeper than {CHAIN}"));
        }
        self.making.push(key);
        let made = self.make(scope, name, e);
        self.making.pop();
        let made = made?;
        self.made.insert(key, made);
        Ok(made)
    }

    /// Node element `e` of `scope`, named `name`, made.
    fn make(&mut self, scope: usize, name: &str, e: &'d Element) -> Result<Made, String> {
        let position = self.position(scope, e);
        let made = match self.doc.own(e) {
            Some((def, graph)) => {
                let def = def.clone();
                let depth = self.scopes[scope].depth + 1;
                if depth > DEPTH {
                    return Err(format!("{name}'s definitions hold one another"));
                }
                let mut interface = HashMap::new();
                for port in &def.inputs {
                    let given = (e.elements())
                        .find(|c| c.category == "input" && c.get("name") == port.name);
                    let bound = match given {
                        Some(input) => self.bind(scope, input, e, &port.ty)?,
                        None => default(port),
                    };
                    interface.insert(port.name.clone(), bound);
                }
                self.scopes.push(Scope {
                    graph: Some(graph),
                    interface,
                    order: position,
                    depth,
                });
                Made::Instance(self.scopes.len() - 1)
            }
            None => {
                let def = node_def(library(), e)?.clone();
                let mut inputs = Vec::new();
                for input in e.elements().filter(|c| c.category == "input") {
                    let Some(port) = def.input(input.get("name")) else {
                        let why = format!(
                            "{} has no input {}, left out",
                            e.get("name"),
                            input.get("name")
                        );
                        self.left.push(why);
                        continue;
                    };
                    if let Some(bound) = self.bind(scope, input, e, &port.ty)? {
                        inputs.push((port.name.clone(), bound));
                    }
                }
                let node = MaterialNode {
                    name: unique(e.get("name").to_owned(), &mut self.taken),
                    def,
                    inputs,
                };
                self.nodes.push((position, node));
                Made::Node(self.nodes.len() - 1)
            }
        };
        Ok(made)
    }

    /// Where `e` of `scope` stands in the document.
    fn position(&self, scope: usize, e: &Element) -> Vec<usize> {
        let within = match self.scopes[scope].graph {
            Some(graph) => graph
                .children
                .iter()
                .position(|c| matches!(c, crate::xml::Child::Element(c) if std::ptr::eq(c, e))),
            None => self
                .doc
                .root
                .children
                .iter()
                .position(|c| matches!(c, crate::xml::Child::Element(c) if std::ptr::eq(c, e))),
        };
        let mut order = self.scopes[scope].order.clone();
        order.push(within.unwrap_or(usize::MAX));
        order
    }

    /// What output `output` (none: the first) of node `name` of `scope`
    /// gives.
    fn output(
        &mut self,
        scope: usize,
        name: &str,
        output: Option<&str>,
    ) -> Result<Option<Bound>, String> {
        match self.node(scope, name)? {
            Made::Node(at) => {
                let def = &self.nodes[at].1.def;
                let port = match output {
                    Some(output) => def.output(output),
                    None => def.outputs.first(),
                };
                let port =
                    port.ok_or_else(|| format!("{name} has no output {}", output.unwrap_or("")))?;
                Ok(Some(Bound::Output {
                    node: at,
                    output: port.name.clone(),
                }))
            }
            Made::Instance(inner) => self.graph_output(inner, output),
        }
    }

    /// What output `output` (none: the first) of the graph of `scope` gives.
    fn graph_output(
        &mut self,
        scope: usize,
        output: Option<&str>,
    ) -> Result<Option<Bound>, String> {
        let graph = self.scopes[scope].graph.expect("a graph's scope");
        let mut outputs = graph.elements().filter(|c| c.category == "output");
        let found = match output {
            Some(name) => outputs.find(|c| c.get("name") == name),
            None => outputs.next(),
        };
        let found = found.ok_or_else(|| {
            format!(
                "{} has no output {}",
                graph.get("name"),
                output.unwrap_or("")
            )
        })?;
        self.bind(scope, found, graph, &Type::of(found.get("type")))
    }

    /// What input (or output) `e` of node `node` in `scope` is bound to, as
    /// a value of `ty`: none when it holds nothing.
    fn bind(
        &mut self,
        scope: usize,
        e: &'d Element,
        node: &'d Element,
        ty: &Type,
    ) -> Result<Option<Bound>, String> {
        if let Some(interface) = e.attr("interfacename") {
            let bound = self.scopes[scope].interface.get(interface).cloned();
            return match (bound, self.scopes[scope].graph) {
                (Some(bound), _) => Ok(bound),
                // A top-level graph's own inputs are its interface.
                (None, Some(graph)) => {
                    let input = graph
                        .elements()
                        .find(|c| c.category == "input" && c.get("name") == interface);
                    match input {
                        Some(input) => self.bind(0, input, graph, ty),
                        None => Err(format!("no interface input {interface}")),
                    }
                }
                (None, None) => Err(format!("{interface}: an interface outside a graph")),
            };
        }
        let read = match (e.attr("nodename"), e.attr("nodegraph")) {
            (Some(name), _) => Some(self.output(scope, name, e.attr("output"))),
            (None, Some(graph)) => Some(
                self.graph(graph)
                    .and_then(|scope| self.graph_output(scope, e.attr("output"))),
            ),
            (None, None) => None,
        };
        if let Some(read) = read {
            // What reads a node that doesn't resolve holds its own value.
            return Ok(read.unwrap_or_else(|why| {
                self.left.push(format!("{why}, left out"));
                None
            }));
        }
        let Some(text) = e.attr("value") else {
            return Ok(None);
        };
        let Some(mut value) = Value::parse(ty, text) else {
            // An empty value of a closure is nothing bound.
            if !text.is_empty() {
                let (node, input) = (node.get("name"), e.get("name"));
                self.left.push(format!(
                    "{node}'s {input} ({text:?} isn't a {ty}), left out"
                ));
            }
            return Ok(None);
        };
        let ancestors = [
            Some(e),
            Some(node),
            self.scopes[scope].graph,
            Some(self.doc.root),
        ];
        let said = |name: &str| ancestors.iter().flatten().find_map(|a| a.attr(name));
        if *ty == Type::Filename
            && let (Value::Text(file), Some(prefix)) = (&mut value, said("fileprefix"))
        {
            file.insert_str(0, prefix);
        }
        let colorspace = match ty.is_color() || *ty == Type::Filename {
            true => said("colorspace")
                .filter(|c| !c.is_empty())
                .map(str::to_owned),
            false => None,
        };
        Ok(Some(Bound::Value { value, colorspace }))
    }

    /// Top-level node graph `name`'s scope, made once.
    fn graph(&mut self, name: &str) -> Result<usize, String> {
        if let Some(&scope) = self.graphs.get(name) {
            return Ok(scope);
        }
        let graph = (self.doc.named.get(name).copied())
            .filter(|e| e.category == "nodegraph")
            .ok_or_else(|| format!("no node graph {name}"))?;
        let order = self.position(0, graph);
        self.scopes.push(Scope {
            graph: Some(graph),
            interface: HashMap::new(),
            order,
            depth: 1,
        });
        let scope = self.scopes.len() - 1;
        self.graphs.insert(graph.get("name"), scope);
        Ok(scope)
    }
}

/// What a definition's input holds when nothing is bound to it.
fn default(port: &Port) -> Option<Bound> {
    let value = port.value.clone()?;
    Some(Bound::Value {
        value,
        colorspace: None,
    })
}
