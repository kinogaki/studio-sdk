//! Material networks: nodes of one engine's vocabulary
//! ([`crate::vocabulary`]), each a type name with its settings and the
//! constants of its unwired inputs, and links between their sockets by
//! name. The network names its engine; only that engine knows what its
//! nodes mean. An engine handed another engine's material renders it flat
//! in that material's tint ([`crate::EngineDescription::adopt`]).
//!
//! Colors are linear: authored in the working space, ACEScg in a network
//! an engine receives (the host converts them as it sends the network,
//! [`MaterialNetwork::map_colors`]; [`crate::color`]).

use std::sync::Arc;

use crate::id::EngineId;
use crate::settings::Textures;
use kinogaki_value::{Parameters, Value};

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
    /// Its key as a choice (`color`, `data`): what a
    /// [`crate::ParamSpec::usage`] setting holds.
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

/// A node: its type's name, its settings, the values of its unwired
/// inputs that differ from their defaults, and, as an engine receives it,
/// the textures its image settings were decoded to.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderNode {
    /// Its type's name in the engine's vocabulary.
    pub kind: Arc<str>,
    /// Its settings, by name.
    pub params: Parameters,
    /// Set inputs' values, by socket name; inputs not listed take their
    /// defaults.
    pub values: Parameters,
    /// Its image settings' Texture prims (filled by the host as it sends
    /// the network).
    pub textures: Textures,
}

impl ShaderNode {
    /// A node of type `kind` with no settings or values set.
    pub fn new(kind: &str) -> ShaderNode {
        ShaderNode {
            kind: Arc::from(kind),
            params: Parameters::new(),
            values: Parameters::new(),
            textures: Textures::new(),
        }
    }

    /// The node with setting `name` set to `value`.
    pub fn with_param(mut self, name: &str, value: impl Into<Value>) -> ShaderNode {
        self.params.insert(name.to_owned(), value.into());
        self
    }

    /// The node with input `name` set to `value`.
    pub fn with(mut self, name: &str, value: impl Into<Value>) -> ShaderNode {
        self.values.insert(name.to_owned(), value.into());
        self
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
    /// The links form a cycle through this node.
    Cycle(NodeIndex),
}

/// A material: one engine's nodes and the links between them.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialNetwork {
    /// The engine whose vocabulary the nodes are of.
    pub engine: EngineId,
    /// The nodes; links name them by index.
    pub nodes: Vec<ShaderNode>,
    /// The links, in the order they were made (engines may emit in it).
    pub links: Vec<Link>,
}

impl MaterialNetwork {
    /// A network of `engine` with no nodes.
    pub fn new(engine: EngineId) -> MaterialNetwork {
        MaterialNetwork {
            engine,
            nodes: Vec::new(),
            links: Vec::new(),
        }
    }

    /// A network of `engine` holding the single node `node`.
    pub fn single(engine: EngineId, node: ShaderNode) -> MaterialNetwork {
        MaterialNetwork {
            engine,
            nodes: vec![node],
            links: Vec::new(),
        }
    }

    /// Every color constant replaced by `f` of it (a host's working space
    /// to ACEScg).
    pub fn map_colors(&mut self, f: impl Fn([f64; 3]) -> [f64; 3]) {
        for node in &mut self.nodes {
            for value in node.values.values_mut() {
                if let Value::Color(c) = value {
                    *c = f(*c);
                }
            }
        }
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
}
