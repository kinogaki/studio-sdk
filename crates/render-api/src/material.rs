//! Material networks: nodes of one engine's vocabulary
//! ([`crate::vocabulary`]), each a type name with its settings and the
//! constants of its unwired inputs, and links between their sockets by
//! name. The network names its engine; only that engine knows what its
//! nodes mean. An engine handed another engine's material renders it flat
//! in that material's tint ([`crate::EngineDescription::adopt`]).
//!
//! Colors are linear ACEScg, the render's space: a host working in another
//! space converts them ([`MaterialNetwork::map_colors`]).

use std::sync::Arc;

use crate::vocabulary::{ParamValue, Params};

/// A node's index in its network.
pub type NodeIndex = u32;

/// How an Image texture's texels are read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageUse {
    /// A color: decoded to linear light (the texture's own color space).
    Color,
    /// Data (roughness, normals): the stored values as they are.
    Data,
}

impl ImageUse {
    /// Its name as a choice (`color`, `data`): what a
    /// [`crate::ParamKind::File`]'s usage setting holds.
    pub fn name(self) -> &'static str {
        match self {
            ImageUse::Color => "color",
            ImageUse::Data => "data",
        }
    }

    /// The use named `name`.
    pub fn of_name(name: &str) -> Option<ImageUse> {
        [ImageUse::Color, ImageUse::Data]
            .into_iter()
            .find(|u| u.name() == name)
    }
}

/// An unwired input's constant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Constant {
    /// One number.
    Number(f32),
    /// A color (ACEScg once it reaches the engine).
    Color([f32; 3]),
    /// Three numbers that are not a color.
    Vector([f32; 3]),
}

impl Constant {
    /// As three numbers (a number spread to three).
    pub fn triple(self) -> [f32; 3] {
        match self {
            Constant::Number(v) => [v; 3],
            Constant::Color(c) | Constant::Vector(c) => c,
        }
    }
}

/// A node: its type's name, its settings, and the constants of its unwired
/// inputs that differ from their defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderNode {
    /// Its type's name in the engine's vocabulary.
    pub kind: Arc<str>,
    /// Its settings, by name.
    pub params: Params,
    /// Set inputs, by socket name; inputs not listed take their defaults.
    pub values: Vec<(Arc<str>, Constant)>,
}

impl ShaderNode {
    /// A node of type `kind` with no settings or values set.
    pub fn new(kind: &str) -> ShaderNode {
        ShaderNode {
            kind: Arc::from(kind),
            params: Vec::new(),
            values: Vec::new(),
        }
    }

    /// The node with setting `name` set to `value`.
    pub fn with_param(mut self, name: &str, value: ParamValue) -> ShaderNode {
        self.params.push((Arc::from(name), value));
        self
    }

    /// The node with input `name` set to `value`.
    pub fn with(mut self, name: &str, value: Constant) -> ShaderNode {
        self.values.push((Arc::from(name), value));
        self
    }

    /// Setting `name`'s value.
    pub fn param(&self, name: &str) -> Option<&ParamValue> {
        crate::vocabulary::param(&self.params, name)
    }

    /// Input `name`'s set constant.
    pub fn value(&self, name: &str) -> Option<Constant> {
        self.values
            .iter()
            .find(|(n, _)| &**n == name)
            .map(|(_, v)| *v)
    }
}

/// One link: output `output` of node `from` drives input `input` of `to`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Link {
    /// The source node.
    pub from: NodeIndex,
    /// The source's output, by name.
    pub output: Arc<str>,
    /// The driven node.
    pub to: NodeIndex,
    /// The driven input, by name.
    pub input: Arc<str>,
}

/// Why a network's links are not usable (what its engine checks beyond
/// this is its own).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkError {
    /// A link names a node past the end.
    NodeOutOfRange(NodeIndex),
    /// An input is linked twice.
    LinkedTwice(NodeIndex, Arc<str>),
    /// The links form a cycle through this node.
    Cycle(NodeIndex),
}

/// A material: one engine's nodes and the links between them.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MaterialNetwork {
    /// The engine whose vocabulary the nodes are of.
    pub engine: Arc<str>,
    /// The nodes; links name them by index.
    pub nodes: Vec<ShaderNode>,
    /// The links, in the order they were made (engines may emit in it).
    pub links: Vec<Link>,
}

impl MaterialNetwork {
    /// A network of `engine` holding the single node `node`.
    pub fn single(engine: &str, node: ShaderNode) -> MaterialNetwork {
        MaterialNetwork {
            engine: Arc::from(engine),
            nodes: vec![node],
            links: Vec::new(),
        }
    }

    /// Every color constant replaced by `f` of it (a host's working space
    /// to ACEScg).
    pub fn map_colors(&mut self, f: impl Fn([f32; 3]) -> [f32; 3]) {
        for node in &mut self.nodes {
            for (_, value) in &mut node.values {
                if let Constant::Color(c) = value {
                    *c = f(*c);
                }
            }
        }
    }

    /// The mesh attributes the network's nodes read, each once, in their
    /// nodes' order (the host sends a mesh these).
    pub fn attribute_names(&self) -> Vec<Arc<str>> {
        let mut names: Vec<Arc<str>> = Vec::new();
        for node in &self.nodes {
            for (_, value) in &node.params {
                if let ParamValue::Attribute(name) = value
                    && !name.is_empty()
                    && !names.contains(name)
                {
                    names.push(name.clone());
                }
            }
        }
        names
    }

    /// The link into input `input` of node `to`, if any.
    pub fn link_into(&self, to: NodeIndex, input: &str) -> Option<&Link> {
        self.links.iter().find(|l| l.to == to && &*l.input == input)
    }

    /// Whether `other` has the same nodes (types and settings) and links,
    /// so only constants differ.
    pub fn same_wiring(&self, other: &MaterialNetwork) -> bool {
        self.engine == other.engine
            && self.links == other.links
            && self.nodes.len() == other.nodes.len()
            && self
                .nodes
                .iter()
                .zip(&other.nodes)
                .all(|(a, b)| a.kind == b.kind && a.params == b.params)
    }

    /// Check the links: every one between nodes the network has, each
    /// input linked once, no cycles.
    pub fn validate_links(&self) -> Result<(), NetworkError> {
        let count = self.nodes.len();
        for (k, link) in self.links.iter().enumerate() {
            for node in [link.from, link.to] {
                if node as usize >= count {
                    return Err(NetworkError::NodeOutOfRange(node));
                }
            }
            if self.links[..k]
                .iter()
                .any(|l| l.to == link.to && l.input == link.input)
            {
                return Err(NetworkError::LinkedTwice(link.to, link.input.clone()));
            }
        }
        // Depth-first over the links: 0 unvisited, 1 on the path, 2 done.
        let mut state = vec![0u8; count];
        for start in 0..count {
            if state[start] != 0 {
                continue;
            }
            let mut stack = vec![(start, 0usize)];
            state[start] = 1;
            while let Some((node, next)) = stack.pop() {
                let sources: Vec<usize> = self
                    .links
                    .iter()
                    .filter(|l| l.to as usize == node)
                    .map(|l| l.from as usize)
                    .collect();
                if next < sources.len() {
                    stack.push((node, next + 1));
                    let source = sources[next];
                    match state[source] {
                        0 => {
                            state[source] = 1;
                            stack.push((source, 0));
                        }
                        1 => return Err(NetworkError::Cycle(source as NodeIndex)),
                        _ => {}
                    }
                } else {
                    state[node] = 2;
                }
            }
        }
        Ok(())
    }
}
