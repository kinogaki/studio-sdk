//! [`NodeDef`]: a material node type as data, the only thing the SDK knows of
//! shading. It says what ports a node has and what values and wires they
//! take; what a node means is its engine's alone. The SDK declares no node:
//! an engine brings its own (a V-Ray plugin its VRayMtl, another engine an
//! OpenPBR surface if it wants one), and its description lists them
//! ([`EngineDescription::node_defs`](crate::EngineDescription::node_defs)). A host makes one node type
//! of each, in the Shading context of a Material.
use crate::settings::{SettingDescriptor, SettingGroup};
use kinogaki_value::{Condition, NodeTypeId, Parameters};
use std::sync::Arc;

/// What a wire between material nodes carries.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Socket {
    /// One number.
    Float,
    /// A color (linear: authored in the working space, ACEScg in the network
    /// an engine receives, [`crate::RENDER_SPACE`]).
    Color,
    /// `width` numbers: a position, direction, normal or texture coordinate.
    Vector {
        /// Numbers a value, 2 to 4.
        width: u8,
    },
    /// Something an engine's own nodes alone agree on, by the name it gives
    /// it (a surface, a volume, a displacement, a closure of its own).
    Closure(Arc<str>),
}

impl Socket {
    /// Whether an output of this socket may drive an input of `input`:
    /// numbers drive numbers (a Float spreads, three-wide values pass, three
    /// into one is the engine's choice); a closure drives its own kind alone.
    pub fn drives(&self, input: &Socket) -> bool {
        match (self, input) {
            (Socket::Closure(a), Socket::Closure(b)) => a == b,
            (Socket::Closure(_), _) | (_, Socket::Closure(_)) => false,
            _ => true,
        }
    }
}

/// One input of a node.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    /// A value no wire drives: an operation, a file, an attribute's name.
    Setting(SettingDescriptor),
    /// A value a wire may drive; unwired, it holds its value.
    Driven(SettingDescriptor, Socket),
    /// A wire alone (a surface into an output node).
    Wire {
        /// Its name in files and networks.
        name: Arc<str>,
        /// What a UI labels it.
        label: Arc<str>,
        /// What it takes.
        socket: Socket,
    },
}

impl Input {
    /// Wire input `name`, labelled `label`, taking `socket`.
    pub fn wire(name: &str, label: &str, socket: Socket) -> Input {
        Input::Wire {
            name: Arc::from(name),
            label: Arc::from(label),
            socket,
        }
    }

    /// Its name in files and networks.
    pub fn name(&self) -> &str {
        match self {
            Input::Setting(d) | Input::Driven(d, _) => &d.name,
            Input::Wire { name, .. } => name,
        }
    }

    /// What a UI labels it.
    pub fn label(&self) -> &str {
        match self {
            Input::Setting(d) | Input::Driven(d, _) => &d.label,
            Input::Wire { label, .. } => label,
        }
    }

    /// The value it holds, unless it is a wire alone.
    pub fn value(&self) -> Option<&SettingDescriptor> {
        match self {
            Input::Setting(d) | Input::Driven(d, _) => Some(d),
            Input::Wire { .. } => None,
        }
    }

    /// What a wire into it carries, when one may be.
    pub fn socket(&self) -> Option<&Socket> {
        match self {
            Input::Setting(_) => None,
            Input::Driven(_, socket) | Input::Wire { socket, .. } => Some(socket),
        }
    }
}

/// One output of a node.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    /// Its name in files and networks.
    pub name: Arc<str>,
    /// What a UI labels it.
    pub label: Arc<str>,
    /// What it carries.
    pub socket: Socket,
}

impl Output {
    /// Output `name`, labelled `label`, carrying `socket`.
    pub fn new(name: &str, label: &str, socket: Socket) -> Output {
        Output {
            name: Arc::from(name),
            label: Arc::from(label),
            socket,
        }
    }
}

/// A material node type: its id, what a UI calls it and its ports.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeDef {
    /// Its id in files, under the vendor of the package declaring it
    /// (`kinogaki.spectre.openpbr_surface`): a host's node type of this id.
    pub id: NodeTypeId,
    /// What a UI calls it, and the stem new nodes are numbered from.
    pub label: Arc<str>,
    /// The family a UI lists it under (`Surface`, `Texture`).
    pub family: Arc<str>,
    /// Words search accepts for it.
    pub aliases: Vec<Arc<str>>,
    /// Its values' groups, in order.
    pub groups: Vec<SettingGroup>,
    /// Its inputs, in the order a UI lists them.
    pub inputs: Vec<Input>,
    /// Its outputs, in slot order.
    pub outputs: Vec<Output>,
    /// The image files read as data (roughness, normals: the stored values as
    /// they are) while their conditions hold; every other file is a color.
    data_files: Vec<(Arc<str>, Condition)>,
}

impl NodeDef {
    /// Node type `id` called `label`, listed under `family`, with no ports.
    pub fn new(id: NodeTypeId, label: &str, family: &str) -> NodeDef {
        NodeDef {
            id,
            label: Arc::from(label),
            family: Arc::from(family),
            aliases: Vec::new(),
            groups: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            data_files: Vec::new(),
        }
    }

    /// With `input` next.
    pub fn input(mut self, input: Input) -> NodeDef {
        self.inputs.push(input);
        self
    }

    /// With `output` next.
    pub fn output(mut self, output: Output) -> NodeDef {
        self.outputs.push(output);
        self
    }

    /// With file `name` read as data while `when` holds.
    pub fn data_file(mut self, name: &str, when: Condition) -> NodeDef {
        self.data_files.push((Arc::from(name), when));
        self
    }

    /// Whether file input `name` reads as data, the node holding `values`.
    pub fn reads_data(&self, name: &str, values: &Parameters) -> bool {
        (self.data_files.iter()).any(|(file, when)| &**file == name && when.met(values))
    }
}
