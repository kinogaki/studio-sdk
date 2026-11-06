//! [`NodeDef`]: a material node type as data. It says what ports a node has
//! and what values and wires they take. MaterialX's nodes are the shared ones
//! ([`super::mtlx`]: one definition a signature, built from the MaterialX
//! library); an engine brings nodes of its own beside them (a V-Ray plugin
//! its VRayMtl) and its description lists them
//! ([`EngineDescription::node_defs`](crate::EngineDescription::node_defs)).
//! A host makes node types of them in the Shading context of a Material.
use kinogaki_mtlx::Type;
use kinogaki_value::{Condition, NodeTypeId, Parameters};
use kinogaki_value::{Parameter, Section};
use std::sync::Arc;

/// What a wire between material nodes carries: MaterialX's types.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Socket {
    /// `boolean`.
    Boolean,
    /// `integer`.
    Integer,
    /// `float`.
    Float,
    /// `color3`: linear (authored in the working space, ACEScg in the network
    /// an engine receives, [`crate::RENDER_SPACE`]).
    Color3,
    /// `color4`: a linear color and its alpha.
    Color4,
    /// `vector2`: a texture coordinate.
    Vector2,
    /// `vector3`: a position, direction or normal.
    Vector3,
    /// `vector4`.
    Vector4,
    /// `matrix33`.
    Matrix33,
    /// `matrix44`.
    Matrix44,
    /// `string`.
    String,
    /// `filename`: an image file.
    Filename,
    /// A closure or shader, by its name: MaterialX's (`BSDF`, `EDF`, `VDF`,
    /// `surfaceshader`, `displacementshader`, `volumeshader`, `material`) or
    /// one an engine's own nodes alone agree on.
    Closure(Arc<str>),
}

impl Socket {
    /// The socket of MaterialX type `ty`.
    pub fn of(ty: &Type) -> Socket {
        match ty {
            Type::Boolean => Socket::Boolean,
            Type::Integer => Socket::Integer,
            Type::Float => Socket::Float,
            Type::Color3 => Socket::Color3,
            Type::Color4 => Socket::Color4,
            Type::Vector2 => Socket::Vector2,
            Type::Vector3 => Socket::Vector3,
            Type::Vector4 => Socket::Vector4,
            Type::Matrix33 => Socket::Matrix33,
            Type::Matrix44 => Socket::Matrix44,
            Type::String => Socket::String,
            Type::Filename => Socket::Filename,
            Type::Named(name) => Socket::Closure(name.clone()),
        }
    }

    /// MaterialX's `surfaceshader`: what a surface's output carries.
    pub fn surface() -> Socket {
        Socket::Closure(Arc::from("surfaceshader"))
    }

    /// The numbers a value of it holds (a color3: 3); 0 for a closure, text
    /// or matrix.
    pub fn width(&self) -> usize {
        match self {
            Socket::Boolean | Socket::Integer | Socket::Float => 1,
            Socket::Vector2 => 2,
            Socket::Color3 | Socket::Vector3 => 3,
            Socket::Color4 | Socket::Vector4 => 4,
            _ => 0,
        }
    }

    /// Whether an output of this socket may drive an input of `input`:
    /// numbers drive numbers (an engine converts between widths, as
    /// MaterialX's `convert` would), text drives text, a matrix one of its
    /// size, and a closure its own kind alone.
    pub fn drives(&self, input: &Socket) -> bool {
        match (self, input) {
            (Socket::Closure(a), Socket::Closure(b)) => a == b,
            (Socket::String | Socket::Filename, Socket::String | Socket::Filename) => true,
            (Socket::Matrix33, Socket::Matrix33) | (Socket::Matrix44, Socket::Matrix44) => true,
            _ => self.width() > 0 && input.width() > 0,
        }
    }
}

/// One input of a node.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    /// A value no wire drives: an operation, a file, an attribute's name
    /// (MaterialX's uniform inputs).
    Setting(Parameter),
    /// A value a wire may drive; unwired, it holds its value.
    Driven(Parameter, Socket),
    /// A wire alone: a closure, or what reads a geometric property while
    /// unwired (MaterialX's `defaultgeomprop`: the shading normal, a texture
    /// coordinate).
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
    pub fn value(&self) -> Option<&Parameter> {
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
    /// Its values' groups, in order.
    pub groups: Vec<Section>,
    /// Its inputs, in the order a UI lists them.
    pub inputs: Vec<Input>,
    /// Its outputs, in slot order.
    pub outputs: Vec<Output>,
    /// The MaterialX definition it is of, for a MaterialX node.
    mtlx: Option<Arc<kinogaki_mtlx::NodeDef>>,
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
            mtlx: None,
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

    /// As the node type of MaterialX definition `def`.
    pub(crate) fn of_mtlx(mut self, def: Arc<kinogaki_mtlx::NodeDef>) -> NodeDef {
        self.mtlx = Some(def);
        self
    }

    /// Whether file input `name` reads as data, the node holding `values`.
    pub fn reads_data(&self, name: &str, values: &Parameters) -> bool {
        (self.data_files.iter()).any(|(file, when)| &**file == name && when.met(values))
    }

    /// The MaterialX definition it is of, for a MaterialX node: its node
    /// (`image`), signature (`ND_image_color3`) and types.
    pub fn mtlx(&self) -> Option<&Arc<kinogaki_mtlx::NodeDef>> {
        self.mtlx.as_ref()
    }

    /// Its input `name`.
    pub fn find_input(&self, name: &str) -> Option<&Input> {
        self.inputs.iter().find(|i| i.name() == name)
    }

    /// Whether it is the same definition as `other`: the same node type, and
    /// for a MaterialX node the same signature.
    pub fn same(&self, other: &NodeDef) -> bool {
        let signature = |d: &NodeDef| d.mtlx.as_ref().map(|m| m.name.clone());
        self.id == other.id && signature(self) == signature(other)
    }
}
