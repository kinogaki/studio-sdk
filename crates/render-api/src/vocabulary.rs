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

use crate::id::TextureId;

/// What a socket carries, for linking and drawing. Every output drives
/// every input: a Float spreads to three, three-wide values pass as they
/// are, three into one is the engine's choice (luminance, average).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SocketKind {
    /// One number.
    Float,
    /// A color (linear, the render's working space: ACEScg).
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

/// How an unwired input is edited.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEdit {
    /// A number in `range`, dragged over `soft` (a slider when `range` is
    /// 0 to 1), shown with `suffix` and `decimals`.
    Number {
        /// The values accepted.
        range: (f64, f64),
        /// The values a drag covers.
        soft: (f64, f64),
        /// The unit shown after the number (" K", " µm"; empty: none).
        suffix: Arc<str>,
        /// Decimals shown.
        decimals: u8,
    },
    /// A color.
    Color,
    /// Three numbers (an axis).
    Vector,
    /// On or off, stored as 1 or 0.
    Toggle,
    /// Not edited: unwired, it means what the text says (the point's
    /// texture coordinates, its shading normal).
    Implicit(Arc<str>),
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
    /// An unwired input's value (outputs: zero).
    pub default: [f32; 3],
    /// How an unwired input is edited.
    pub edit: InputEdit,
    /// The inspector section it sits in (none: the node's own rows).
    pub section: Option<Arc<str>>,
    /// What it means, where its label alone does not say.
    pub tooltip: Option<Arc<str>>,
    /// Whether the inspector shows it for the node's settings (it stays a
    /// socket either way).
    pub shown: bool,
}

impl SocketSpec {
    /// A socket named `name`, labelled `label`, of `kind` with `default`;
    /// edited as its kind suggests, shown, in no section.
    pub fn new(name: &str, label: &str, kind: SocketKind, default: [f32; 3]) -> SocketSpec {
        SocketSpec {
            name: Arc::from(name),
            label: Arc::from(label),
            kind,
            default,
            edit: match kind {
                SocketKind::Float => InputEdit::Number {
                    range: (0.0, 1.0),
                    soft: (0.0, 1.0),
                    suffix: Arc::from(""),
                    decimals: 3,
                },
                SocketKind::Color => InputEdit::Color,
                SocketKind::Vector | SocketKind::Uv => InputEdit::Vector,
            },
            section: None,
            tooltip: None,
            shown: true,
        }
    }
}

/// A named choice: its name in files, its label in a UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// Its name in files (`bead_blasted`).
    pub name: Arc<str>,
    /// What a UI shows (`Bead Blasted`).
    pub label: Arc<str>,
}

impl Choice {
    /// The choice `name` labelled `label`.
    pub fn new(name: &str, label: &str) -> Choice {
        Choice {
            name: Arc::from(name),
            label: Arc::from(label),
        }
    }
}

/// What a node setting holds.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    /// One of named choices; `menu` shows them as a dropdown (else as
    /// segments).
    Choice {
        /// The choices.
        choices: Vec<Choice>,
        /// A dropdown rather than segments.
        menu: bool,
    },
    /// A whole number in `min..=max`.
    Count {
        /// The smallest.
        min: u32,
        /// The largest.
        max: u32,
    },
    /// The name of a mesh attribute read at the shaded point. A node whose
    /// name is empty reads nothing: the host leaves it out, or, for a
    /// surface (a material's terminal), sends the name empty.
    Attribute,
    /// An image file, read as color or data by the choice setting `usage`
    /// (its choices named `color` and `data`). The host decodes it and
    /// sends a Texture prim; a node whose image is not available is left
    /// out.
    File {
        /// The setting that says how its texels are read.
        usage: Arc<str>,
    },
}

/// A node setting's value.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ParamValue {
    /// A choice, by name.
    Choice(Arc<str>),
    /// A whole number.
    Count(u32),
    /// A mesh attribute's name.
    Attribute(Arc<str>),
    /// An image file's path, as typed.
    File(Arc<str>),
    /// The Texture prim a file was decoded to (what an engine receives).
    Texture(TextureId),
}

impl ParamValue {
    /// A choice's name.
    pub fn choice(&self) -> Option<&str> {
        match self {
            ParamValue::Choice(name) => Some(name),
            _ => None,
        }
    }

    /// A count.
    pub fn count(&self) -> Option<u32> {
        match self {
            ParamValue::Count(n) => Some(*n),
            _ => None,
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
    /// What it holds.
    pub kind: ParamKind,
    /// A new node's value.
    pub default: ParamValue,
    /// What it means, where its label alone does not say.
    pub tooltip: Option<Arc<str>>,
    /// Whether the inspector shows it for the node's other settings.
    pub shown: bool,
}

impl ParamSpec {
    /// Whether `value` fits it (a choice it offers, a count in range).
    pub fn accepts(&self, value: &ParamValue) -> bool {
        match (&self.kind, value) {
            (ParamKind::Choice { choices, .. }, ParamValue::Choice(name)) => {
                choices.iter().any(|c| c.name == *name)
            }
            (ParamKind::Count { min, max }, ParamValue::Count(n)) => (*min..=*max).contains(n),
            (ParamKind::Attribute, ParamValue::Attribute(_)) => true,
            (ParamKind::File { .. }, ParamValue::File(_) | ParamValue::Texture(_)) => true,
            _ => false,
        }
    }
}

/// A node's settings, by name.
pub type Params = Vec<(Arc<str>, ParamValue)>;

/// The value of setting `name` in `params`.
pub fn param<'a>(params: &'a Params, name: &str) -> Option<&'a ParamValue> {
    params.iter().find(|(n, _)| &**n == name).map(|(_, v)| v)
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
pub type Layout = Arc<dyn Fn(&str, &Params) -> NodeLayout + Send + Sync>;

/// A node type of an engine's vocabulary.
#[derive(Clone)]
pub struct NodeType {
    /// The engine it belongs to (its [`crate::EngineDescription::id`]).
    pub engine: Arc<str>,
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
    pub defaults: Params,
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
    pub fn layout(&self, params: &Params) -> NodeLayout {
        (self.layout)(&self.name, params)
    }

    /// `params` made to fit the type: each setting its layout offers, with
    /// its value from `params` when that fits, else its default. A setting
    /// can change which others there are (a Convert's types and its
    /// channel), so this repeats until the layout stands.
    pub fn fit(&self, params: &Params) -> Params {
        let mut current = params.clone();
        for _ in 0..4 {
            let layout = self.layout(&current);
            let fitted: Params = layout
                .params
                .iter()
                .map(|spec| {
                    let value = param(&current, &spec.name)
                        .filter(|v| spec.accepts(v))
                        .cloned()
                        .unwrap_or_else(|| spec.default.clone());
                    (spec.name.clone(), value)
                })
                .collect();
            if fitted == current {
                break;
            }
            current = fitted;
        }
        current
    }
}
