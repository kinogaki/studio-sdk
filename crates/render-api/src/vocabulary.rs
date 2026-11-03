//! An engine's material vocabulary, as data a host builds its UI from: the
//! node types an engine offers ([`NodeType`]), each with its settings
//! ([`ParamSpec`]: what is not a socket, such as a Math node's operation)
//! and its sockets ([`SocketSpec`]). A type's settings can change its
//! sockets (a Switch's count, a Convert's types), so a type gives its
//! [`NodeLayout`] for the settings a node holds.
//!
//! The host stores nodes by type name, settings by name and inputs by
//! socket name ([`crate::MaterialNetwork`]); only the engine that owns the
//! vocabulary knows what they mean.

use std::sync::Arc;

use crate::id::EngineId;
use kinogaki_value::{Parameters, Value, ValueKind};

/// What a socket carries, for linking and drawing. Every output drives
/// every input: a Float spreads to three, three-wide values pass as they
/// are, three into one is the engine's choice (luminance, average).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SocketKind {
    /// One number.
    Float,
    /// A color (linear: authored in the working space, ACEScg in the
    /// network an engine receives, [`crate::RENDER_SPACE`]).
    Color,
    /// A vector: a position, direction or normal.
    Vector,
    /// Texture coordinates.
    Uv,
}

impl SocketKind {
    /// Floats a value takes.
    pub fn width(self) -> u32 {
        match self {
            SocketKind::Float => 1,
            _ => 3,
        }
    }
}

/// One socket of a node.
#[derive(Clone, Debug, PartialEq)]
pub struct SocketSpec {
    /// Its name in files and networks (`base_color`).
    pub name: Arc<str>,
    /// What a UI labels it (`Base Color`).
    pub label: Arc<str>,
    /// What it carries.
    pub kind: SocketKind,
    /// An unwired input's value (a [`Value::Number`] for a Float, a
    /// [`Value::Color`] for a Color, three [`Value::Numbers`] for a
    /// Vector or Uv); an output's is its kind's zero.
    pub default: Value,
    /// The inspector section it sits in (none: the node's own rows).
    pub section: Option<Arc<str>>,
    /// What it means, where its label alone does not say.
    pub tooltip: Option<Arc<str>>,
    /// Whether the inspector shows it for the node's settings (it stays a
    /// socket either way).
    pub shown: bool,
}

impl SocketSpec {
    /// A socket named `name`, labelled `label`, of `kind` with `default`
    /// (an unwired input is edited as its kind is), shown, in no section.
    pub fn new(name: &str, label: &str, kind: SocketKind, default: impl Into<Value>) -> SocketSpec {
        SocketSpec {
            name: Arc::from(name),
            label: Arc::from(label),
            kind,
            default: default.into(),
            section: None,
            tooltip: None,
            shown: true,
        }
    }
}

/// A node setting.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamSpec {
    /// Its name in files (`operation`).
    pub name: Arc<str>,
    /// What a UI labels it.
    pub label: Arc<str>,
    /// What it holds: a choice, an integer, an attribute's name (read at
    /// the shaded point; empty reads nothing: the host leaves the node out,
    /// or, for a surface, sends the name empty), or an image file (the host
    /// decodes it and sends its Texture prim; a node whose image is not
    /// available is left out).
    pub kind: ValueKind,
    /// A new node's value.
    pub default: Value,
    /// For an image file: the choice setting that says how its texels are
    /// read (its choices keyed `color` and `data`, [`crate::ImageUse`]).
    pub usage: Option<Arc<str>>,
    /// What it means, where its label alone does not say.
    pub tooltip: Option<Arc<str>>,
    /// Whether the inspector shows it for the node's other settings.
    pub shown: bool,
}

impl ParamSpec {
    /// Whether `value` fits it (a choice it offers, a count in range).
    pub fn accepts(&self, value: &Value) -> bool {
        self.kind.check(value).is_ok()
    }
}

/// A node's settings and sockets, for the settings it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeLayout {
    /// Its settings, in the inspector's order.
    pub params: Vec<ParamSpec>,
    /// Its inputs, in operand order.
    pub inputs: Vec<SocketSpec>,
    /// Its outputs, in slot order.
    pub outputs: Vec<SocketSpec>,
}

impl NodeLayout {
    /// Input `name`'s index.
    pub fn input(&self, name: &str) -> Option<usize> {
        self.inputs.iter().position(|s| &*s.name == name)
    }

    /// Output `name`'s index.
    pub fn output(&self, name: &str) -> Option<usize> {
        self.outputs.iter().position(|s| &*s.name == name)
    }
}

/// A node type's settings and sockets for the settings a node holds,
/// given the type's name (settings missing take their defaults).
pub type Layout = Arc<dyn Fn(&str, &Parameters) -> NodeLayout + Send + Sync>;

/// A node type of an engine's vocabulary.
#[derive(Clone)]
pub struct NodeType {
    /// The engine it belongs to (its [`crate::EngineDescription::id`]).
    pub engine: EngineId,
    /// Its name in files and networks (`openpbr_surface`).
    pub name: Arc<str>,
    /// What a UI calls it, and the stem new nodes are numbered from.
    pub label: Arc<str>,
    /// The category a UI lists it under (`Surface`, `Texture`).
    pub category: Arc<str>,
    /// Words search accepts for it.
    pub aliases: Vec<Arc<str>>,
    /// For a surface (a material's terminal), the material type's name
    /// (`OpenPBR`); none for every other node.
    pub surface: Option<Arc<str>>,
    /// A new node's settings.
    pub defaults: Parameters,
    /// Its settings and sockets for `params`.
    pub layout: Layout,
}

impl std::fmt::Debug for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeType({}.{})", self.engine, self.name)
    }
}

impl PartialEq for NodeType {
    /// Types are equal when they are the same engine's same type.
    fn eq(&self, other: &NodeType) -> bool {
        self.engine == other.engine && self.name == other.name
    }
}

impl NodeType {
    /// Its settings and sockets for `params`.
    pub fn layout(&self, params: &Parameters) -> NodeLayout {
        (self.layout)(&self.name, params)
    }

    /// `params` made to fit the type: each setting its layout offers, with
    /// its value from `params` when that fits, else its default. A setting
    /// can change which others there are (a Convert's types and its
    /// channel), so this repeats until the layout stands: a layout settles
    /// once each setting it offers fits, so the most rounds are one more
    /// than its settings. A layout that never settles is the type's error,
    /// reported as `None`.
    pub fn fit(&self, params: &Parameters) -> Option<Parameters> {
        let mut current = params.clone();
        for _ in 0..=current.len().max(self.defaults.len()) + 1 {
            let layout = self.layout(&current);
            let fitted: Parameters = (layout.params.iter())
                .map(|spec| {
                    let value = (current.get(&*spec.name))
                        .filter(|v| spec.accepts(v))
                        .unwrap_or(&spec.default);
                    (spec.name.to_string(), value.clone())
                })
                .collect();
            if fitted == current {
                return Some(current);
            }
            current = fitted;
        }
        None
    }
}
