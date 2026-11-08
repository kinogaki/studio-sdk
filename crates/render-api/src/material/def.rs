//! [`NodeDef`]: a material node type as data. It says what ports a node has
//! and what values and wires they take. MaterialX's nodes are the shared ones
//! ([`super::mtlx`]: one definition a signature, built from the MaterialX
//! library); an engine brings nodes of its own beside them (a V-Ray plugin
//! its VRayMtl) and its description lists them
//! ([`EngineDescription::node_defs`](crate::EngineDescription::node_defs)).
//! A host makes node types of them in the Shading context of a Material.
//!
//! What a port carries is a MaterialX [`Type`]: a value (`float`, `color3`,
//! ...), an image (`filename`: wired from the host's Image node; a network an
//! engine receives holds no wire into it, but the image's Texture prim, in
//! its node's [`textures`](super::Node::textures)), or a closure or shader
//! by its name (MaterialX's `BSDF`, `surfaceshader`, ...; or one an
//! engine's own nodes alone agree on, a [`Type::Named`] of its own). Colors
//! are linear: authored in the working space, ACEScg in the network an
//! engine receives ([`crate::RENDER_SPACE`]).
use kinogaki_mtlx::Type;
use kinogaki_value::NodeTypeId;
use kinogaki_value::{Parameter, Schema, Section};
use std::sync::Arc;

/// One input of a node.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeInput {
    /// A value no wire drives: an operation, an attribute's name (MaterialX's
    /// uniform inputs).
    Setting(Parameter),
    /// A value a wire may drive; unwired, it holds its value.
    Driven(Parameter, Type),
    /// A wire alone: a closure, an image, or what reads a geometric property
    /// while unwired (MaterialX's `defaultgeomprop`: the shading normal, a
    /// texture coordinate).
    Wire {
        /// Its name in files and networks.
        name: Arc<str>,
        /// What a UI labels it.
        label: Arc<str>,
        /// What it takes.
        socket: Type,
    },
}

impl NodeInput {
    /// Wire input `name`, labelled `label`, taking `socket`.
    pub fn wire(name: &str, label: &str, socket: Type) -> NodeInput {
        NodeInput::Wire {
            name: Arc::from(name),
            label: Arc::from(label),
            socket,
        }
    }

    /// Its name in files and networks.
    pub fn name(&self) -> &str {
        match self {
            NodeInput::Setting(d) | NodeInput::Driven(d, _) => &d.name,
            NodeInput::Wire { name, .. } => name,
        }
    }

    /// What a UI labels it.
    pub fn label(&self) -> &str {
        match self {
            NodeInput::Setting(d) | NodeInput::Driven(d, _) => &d.label,
            NodeInput::Wire { label, .. } => label,
        }
    }

    /// The value it holds, unless it is a wire alone.
    pub fn value(&self) -> Option<&Parameter> {
        match self {
            NodeInput::Setting(d) | NodeInput::Driven(d, _) => Some(d),
            NodeInput::Wire { .. } => None,
        }
    }

    /// What a wire into it carries, when one may be.
    pub fn socket(&self) -> Option<&Type> {
        match self {
            NodeInput::Setting(_) => None,
            NodeInput::Driven(_, socket) | NodeInput::Wire { socket, .. } => Some(socket),
        }
    }
}

/// One output of a node.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeOutput {
    /// Its name in files and networks.
    pub name: Arc<str>,
    /// What a UI labels it.
    pub label: Arc<str>,
    /// What it carries.
    pub socket: Type,
}

impl NodeOutput {
    /// Output `name`, labelled `label`, carrying `socket`.
    pub fn new(name: &str, label: &str, socket: Type) -> NodeOutput {
        NodeOutput {
            name: Arc::from(name),
            label: Arc::from(label),
            socket,
        }
    }
}

/// A material node type: its id, what a UI calls it and its ports; for a
/// MaterialX node, the definition (signature) it is of.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeDef {
    /// Its id in files: under the vendor of the package declaring it
    /// (`kinogaki.spectre.plastic_surface`), or a MaterialX node's
    /// (`kinogaki.mtlx.image`).
    pub id: NodeTypeId,
    /// What a UI calls it, and the stem new nodes are numbered from.
    pub label: Arc<str>,
    /// The family a UI lists it under (`Surface`, `texture2d`).
    pub family: Arc<str>,
    /// Words search accepts for it.
    pub aliases: Vec<Arc<str>>,
    /// The sections its values sit in, by their groups.
    sections: Vec<Section>,
    /// Its inputs, in the order a UI lists them.
    pub inputs: Vec<NodeInput>,
    /// Its outputs, in slot order.
    pub outputs: Vec<NodeOutput>,
    /// The MaterialX definition it is of, for a MaterialX node.
    mtlx: Option<Arc<kinogaki_mtlx::NodeDef>>,
}

impl NodeDef {
    /// Node type `id` called `label`, listed under `family`, with no ports.
    pub fn new(id: NodeTypeId, label: &str, family: &str) -> NodeDef {
        NodeDef {
            id,
            label: Arc::from(label),
            family: Arc::from(family),
            aliases: Vec::new(),
            sections: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            mtlx: None,
        }
    }

    /// With `input` next.
    pub fn input(mut self, input: NodeInput) -> NodeDef {
        self.inputs.push(input);
        self
    }

    /// With `output` next.
    pub fn output(mut self, output: NodeOutput) -> NodeDef {
        self.outputs.push(output);
        self
    }

    /// With `section` after its sections: what its values' groups name.
    /// One named as a section it has is left out.
    pub fn section(mut self, section: Section) -> NodeDef {
        if !self.sections.iter().any(|s| s.name == section.name) {
            self.sections.push(section);
        }
        self
    }

    /// Its values as rows: its sections, and each input holding a value, in
    /// input order.
    pub fn schema(&self) -> Schema {
        let schema = (self.sections.iter().cloned()).fold(Schema::default(), Schema::section);
        (self.inputs.iter())
            .filter_map(NodeInput::value)
            .cloned()
            .fold(schema, Schema::parameter)
    }

    /// As the node type of MaterialX definition `def`.
    pub(crate) fn of_mtlx(mut self, def: Arc<kinogaki_mtlx::NodeDef>) -> NodeDef {
        self.mtlx = Some(def);
        self
    }

    /// The MaterialX definition it is of, for a MaterialX node: its node
    /// (`image`), signature (`ND_image_color3`) and types.
    pub fn mtlx(&self) -> Option<&Arc<kinogaki_mtlx::NodeDef>> {
        self.mtlx.as_ref()
    }

    /// Its input `name`.
    pub fn find_input(&self, name: &str) -> Option<&NodeInput> {
        self.inputs.iter().find(|i| i.name() == name)
    }

    /// Whether it is the same definition as `other`: the same node type, and
    /// for a MaterialX node the same signature.
    pub fn same(&self, other: &NodeDef) -> bool {
        let signature = |d: &NodeDef| d.mtlx.as_ref().map(|m| m.name.clone());
        self.id == other.id && signature(self) == signature(other)
    }
}
