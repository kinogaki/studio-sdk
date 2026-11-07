//! [`Work`]: a network being rewritten: nodes by a stable index (a removed
//! one gone), links by name, the shown node. Expansion, translation and the
//! simplifications edit it; [`Work::finish`] makes it a checked network again,
//! keeping only what the shown node reads.
use super::super::mtlx::{self, value};
use super::super::{Link, MaterialNetwork, Node, NodeDef, NodeIndex};
use crate::Id;
use crate::geometry::image::{Image, ImageSource};
use crate::settings::Textures;
use kinogaki_mtlx::{Binding, GeomProp, Graph, library};
use kinogaki_value::{Parameters, Value, ValueKind};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// A network being rewritten (see the module docs).
pub(super) struct Work {
    pub nodes: Vec<Option<Node>>,
    pub links: Vec<Link>,
    pub output: Option<NodeIndex>,
    pub cut: BTreeSet<Arc<str>>,
}

/// Where an input's value comes from, as an outer node held it: a link, a
/// value, an image (with its Texture prim once the host read it), or nothing
/// (a wire alone, unwired).
enum Source {
    Link(NodeIndex, Arc<str>),
    Value(Value),
    Image(ImageSource, Option<Id<Image>>),
    Nothing,
}

impl Work {
    pub fn of(network: &MaterialNetwork) -> Work {
        Work {
            nodes: network.nodes.iter().cloned().map(Some).collect(),
            links: network.links.clone(),
            output: network.output,
            cut: network.cut.iter().cloned().collect(),
        }
    }

    pub fn node(&self, at: NodeIndex) -> Option<&Node> {
        self.nodes.get(at as usize)?.as_ref()
    }

    /// The live nodes' indices.
    pub fn live(&self) -> Vec<NodeIndex> {
        (0..self.nodes.len() as NodeIndex)
            .filter(|&n| self.node(n).is_some())
            .collect()
    }

    /// A node of `def` at its defaults.
    pub fn add(&mut self, def: Arc<NodeDef>) -> NodeIndex {
        let values = (def.inputs.iter())
            .filter_map(|i| i.value())
            .map(|p| (p.name.clone(), p.value.clone()))
            .collect();
        self.nodes.push(Some(Node {
            def,
            values,
            images: BTreeMap::new(),
            textures: Textures::new(),
        }));
        (self.nodes.len() - 1) as NodeIndex
    }

    pub fn link(&mut self, from: NodeIndex, output: &str, to: NodeIndex, input: &str) {
        self.links.retain(|l| !(l.to == to && &*l.input == input));
        self.links.push(Link {
            from,
            output: Arc::from(output),
            to,
            input: Arc::from(input),
        });
    }

    /// The link into input `input` of `to`.
    pub fn driver(&self, to: NodeIndex, input: &str) -> Option<&Link> {
        self.links.iter().find(|l| l.to == to && &*l.input == input)
    }

    /// Whether any link enters `at`.
    pub fn driven(&self, at: NodeIndex) -> bool {
        self.links.iter().any(|l| l.to == at)
    }

    /// The links leaving `at`, as (its output, the driven node, its input).
    pub fn out_of(&self, at: NodeIndex) -> Vec<(Arc<str>, NodeIndex, Arc<str>)> {
        (self.links.iter())
            .filter(|l| l.from == at)
            .map(|l| (l.output.clone(), l.to, l.input.clone()))
            .collect()
    }

    /// The nodes `output` reads, itself included.
    fn read(&self, output: NodeIndex) -> BTreeSet<NodeIndex> {
        let mut read = BTreeSet::from([output]);
        let mut open = vec![output];
        while let Some(at) = open.pop() {
            for l in self.links.iter().filter(|l| l.to == at) {
                if self.node(l.from).is_some() && read.insert(l.from) {
                    open.push(l.from);
                }
            }
        }
        read
    }

    /// Removes every node the shown one doesn't read.
    pub fn prune(&mut self) {
        let Some(output) = self.output.filter(|&o| self.node(o).is_some()) else {
            return;
        };
        let read = self.read(output);
        for at in self.live().into_iter().filter(|at| !read.contains(at)) {
            self.take(at);
        }
    }

    /// Removes `at` and every link to or from it; its node.
    pub fn take(&mut self, at: NodeIndex) -> Option<Node> {
        self.links.retain(|l| l.from != at && l.to != at);
        self.nodes.get_mut(at as usize)?.take()
    }

    /// Sets input `input` of `at` to `v`, when it holds a value that takes it.
    pub fn set(&mut self, at: NodeIndex, input: &str, v: Value) -> bool {
        let Some(node) = self.nodes.get_mut(at as usize).and_then(Option::as_mut) else {
            return false;
        };
        let takes = (node.def.find_input(input))
            .and_then(|i| i.value())
            .is_some_and(|p| p.kind.accepts(&v));
        if takes {
            node.values.insert(input.to_owned(), v);
        }
        takes
    }

    /// Sets input `input` of `at` to MaterialX value `v`, as its kind holds it.
    pub fn set_mtlx(&mut self, at: NodeIndex, input: &str, v: &kinogaki_mtlx::Value) -> bool {
        let kind = (self.node(at))
            .and_then(|n| n.def.find_input(input)?.value())
            .map(|p| p.kind.clone());
        match kind.and_then(|k| value(&k, v)) {
            Some(v) => self.set(at, input, v),
            None => false,
        }
    }

    /// What drives input `input` of `node` (at `at`), before it goes.
    fn source(&self, at: NodeIndex, node: &Node, input: &str) -> Source {
        if let Some(l) = self.driver(at, input) {
            return Source::Link(l.from, l.output.clone());
        }
        if let Some(image) = node.images.get(input) {
            return Source::Image(image.clone(), node.textures.get(input).copied());
        }
        match node.values.get(input) {
            Some(v) => Source::Value(v.clone()),
            None => Source::Nothing,
        }
    }

    /// Input `input` of `to` driven by `source`; `geomprop` is what the outer
    /// input read unwired, which a node reads in its place when `to`'s input
    /// reads another.
    fn bind(&mut self, source: &Source, geomprop: Option<&str>, to: NodeIndex, input: &str) {
        match source {
            Source::Link(from, output) => self.link(*from, output, to, input),
            Source::Value(v) => _ = self.set(to, input, v.clone()),
            Source::Image(image, texture) => {
                if let Some(node) = self.nodes.get_mut(to as usize).and_then(Option::as_mut) {
                    node.images.insert(input.to_owned(), image.clone());
                    if let Some(texture) = texture {
                        node.textures.insert(input.to_owned(), *texture);
                    }
                }
            }
            Source::Nothing => {
                let reads = (self.node(to).and_then(|n| n.def.mtlx()))
                    .and_then(|d| d.input(input)?.geomprop.clone());
                if let Some(g) = geomprop.filter(|g| reads.as_deref() != Some(*g))
                    && let Some(reader) = self.geomprop(g)
                {
                    self.link(reader, "out", to, input);
                }
            }
        }
    }

    /// A node reading geometric property `name` (`Nworld`), as MaterialX
    /// reads an unconnected input's.
    fn geomprop(&mut self, name: &str) -> Option<NodeIndex> {
        let prop: &GeomProp = library().geomprop(name)?;
        let def = library()
            .signatures(&prop.node)
            .find(|d| d.ty() == prop.ty)?;
        let at = self.add(mtlx::def(&def.name)?);
        if let Some(space) = &prop.space {
            self.set(at, "space", Value::Choice(Arc::from(space.as_str())));
        }
        if let Some(index) = prop.index {
            self.set(at, "index", Value::Integer(index));
        }
        Some(at)
    }

    /// Node `at` replaced by `graph`, the graph its definition is: its inputs'
    /// sources bound into the graph's interface, its outputs' readers to what
    /// yields them. The new nodes, or none when a node of the graph has no
    /// definition here (nothing changes then).
    pub fn expand(&mut self, at: NodeIndex, graph: &Graph) -> Option<Vec<NodeIndex>> {
        let defs: Option<Vec<Arc<NodeDef>>> = (graph.nodes.iter())
            .map(|g| mtlx::def(&g.def.name))
            .collect();
        let defs = defs?;
        let outer = self.node(at)?.clone();
        let outer_def = outer.def.mtlx()?.clone();
        let sources: Vec<(String, Source, Option<String>)> = (outer_def.inputs.iter())
            .map(|p| {
                (
                    p.name.clone(),
                    self.source(at, &outer, &p.name),
                    p.geomprop.clone(),
                )
            })
            .collect();
        let readers = self.out_of(at);
        self.take(at);
        let interface = |name: &str| sources.iter().find(|(n, ..)| n == name);
        let made: Vec<NodeIndex> = defs.into_iter().map(|d| self.add(d)).collect();
        for (g, &to) in graph.nodes.iter().zip(&made) {
            for (input, binding) in &g.inputs {
                match binding {
                    Binding::Value(v) => _ = self.set_mtlx(to, input, v),
                    Binding::Output { node, output } => self.link(made[*node], output, to, input),
                    Binding::Interface(name) => {
                        if let Some((_, source, geomprop)) = interface(name) {
                            self.bind(source, geomprop.as_deref(), to, input);
                        }
                    }
                }
            }
        }
        for (output, reader, input) in readers {
            let Some((_, binding)) = graph.outputs.iter().find(|(o, _)| **o == *output) else {
                continue;
            };
            match binding {
                Binding::Value(v) => _ = self.set_mtlx(reader, &input, v),
                Binding::Output { node, output } => self.link(made[*node], output, reader, &input),
                Binding::Interface(name) => {
                    if let Some((_, source, geomprop)) = interface(name) {
                        self.bind(source, geomprop.as_deref(), reader, &input);
                    }
                }
            }
        }
        if self.output == Some(at) {
            self.output = graph.outputs.first().and_then(|(_, b)| match b {
                Binding::Output { node, .. } => Some(made[*node]),
                _ => None,
            });
        }
        Some(made)
    }

    /// Surface `at` translated by `step` (`<from>_to_<to>`) into a surface of
    /// `target`: a translation node taking the surface's inputs by name, the
    /// target surface reading its `<input>_out` outputs, the surface's
    /// readers reading the target. The two new nodes.
    pub fn translate(
        &mut self,
        at: NodeIndex,
        step: Arc<NodeDef>,
        target: Arc<NodeDef>,
    ) -> Option<[NodeIndex; 2]> {
        let outer = self.node(at)?.clone();
        let outer_def = outer.def.mtlx()?.clone();
        let sources: Vec<(String, Source)> = (outer_def.inputs.iter())
            .map(|p| (p.name.clone(), self.source(at, &outer, &p.name)))
            .collect();
        let readers = self.out_of(at);
        self.take(at);
        let translation = self.add(step.clone());
        for (name, source) in &sources {
            if step.find_input(name).is_some() {
                self.bind(source, None, translation, name);
            }
        }
        let made = self.add(target.clone());
        for output in &step.outputs {
            let Some(input) = output.name.strip_suffix("_out") else {
                continue;
            };
            if target.find_input(input).is_some() {
                self.link(translation, &output.name, made, input);
            }
        }
        let out = target.outputs.first().map(|o| o.name.clone())?;
        for (_, reader, input) in readers {
            self.link(made, &out, reader, &input);
        }
        if self.output == Some(at) {
            self.output = Some(made);
        }
        Some([translation, made])
    }

    /// The network: the nodes the shown one reads, renumbered in order, the
    /// primvars their values name, and what was cut.
    pub fn finish(self) -> MaterialNetwork {
        let Some(output) = self.output.filter(|&o| self.node(o).is_some()) else {
            return MaterialNetwork {
                cut: self.cut.into_iter().collect(),
                ..MaterialNetwork::default()
            };
        };
        let read = self.read(output);
        let index: Vec<Option<NodeIndex>> = {
            let mut next = 0;
            (0..self.nodes.len() as NodeIndex)
                .map(|n| {
                    read.contains(&n).then(|| {
                        next += 1;
                        next - 1
                    })
                })
                .collect()
        };
        let renumber = |n: NodeIndex| index[n as usize];
        let links = (self.links.iter())
            .filter_map(|l| {
                Some(Link {
                    from: renumber(l.from)?,
                    to: renumber(l.to)?,
                    ..l.clone()
                })
            })
            .collect();
        let nodes: Vec<Node> = (self.nodes.into_iter().enumerate())
            .filter(|(n, _)| read.contains(&(*n as NodeIndex)))
            .filter_map(|(_, node)| node)
            .collect();
        let mut primvars = BTreeSet::new();
        for node in &nodes {
            for p in node.def.inputs.iter().filter_map(|i| i.value()) {
                if let (ValueKind::Attribute { .. }, Some(Value::Text(names))) =
                    (&p.kind, node.values.get(&p.name))
                {
                    primvars.extend(names.split_whitespace().map(Arc::from));
                }
            }
        }
        MaterialNetwork {
            nodes,
            links,
            output: renumber(output),
            primvars: primvars.into_iter().collect(),
            cut: self.cut.into_iter().collect(),
        }
    }
}

/// `values` of `node`'s inputs as MaterialX values, by name.
pub(super) fn mtlx_values(
    values: &Parameters,
) -> impl Fn(&str) -> Option<kinogaki_mtlx::Value> + '_ {
    |name| values.get(name).and_then(mtlx::of_value)
}
