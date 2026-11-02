//! Shader node types: an engine's material vocabulary
//! ([`kinogaki_render_api::NodeType`]) as SDK node types of Studio's
//! Shading context, what a Material's network is made of. Each engine's
//! package registers its own ([`shaders`]); the Shading compiler finds a
//! node's [`ShaderType`] through
//! [`CompileCx::node_type`](crate::network::CompileCx::node_type) and
//! reads its engine's vocabulary from it ([`ShaderType::vocabulary`]).
//!
//! A node holds its type's settings as parameters (a choice by its index,
//! a count as a number, an attribute's name and an image's path as text)
//! and one constant parameter per edited input socket, named by the
//! socket, which the socket reads while unwired. A type's settings can
//! change its sockets (a Switch's count), so its ports and the parameters
//! beyond its default layout's follow the node's settings
//! ([`NodeType::ports`], [`NodeType::spares`]).
use crate::cook::{
    Category, CookCx, CookError, Description, NodeType, Outputs, Port, Ports, PortsCx, SchemaCx,
};
use crate::data::Data;
use crate::error::Error;
use crate::id::{ContextId, DataTypeId, NodeTypeId, PackageId};
use crate::network::{NetworkSeed, NewNode, NodeCreation};
use crate::params::{Number, Parameter, ParameterKind, Parameters, Schema, Value};
use kinogaki_render_api::vocabulary::{
    self as vocabulary, InputEdit, NodeLayout, ParamKind, ParamSpec, ParamValue, Params,
    SocketSpec, param,
};
use kinogaki_render_api::{EngineDescription, MaterialNetwork, MaterialPreset};
use std::collections::BTreeSet;
use std::sync::Arc;

/// Studio's Shading context, which shader node types are offered in.
pub const SHADING: &str = "kinogaki.core.shading";

/// Studio's Material node type, which holds a Shading network.
pub const MATERIAL: &str = "kinogaki.core.material";

/// The data type every shader socket carries: every output drives every
/// input (a number spreads to three, three into one is the engine's
/// choice), so one type wires them all. The render package registers it.
pub const SHADER_VALUE: &str = "kinogaki.render.shader_value";

/// The data type of shader sockets ([`SHADER_VALUE`]).
pub fn shader_value() -> DataTypeId {
    DataTypeId::declared(SHADER_VALUE)
}

/// One node type of an engine's vocabulary as an SDK node type in the
/// Shading context, named under the package registering it
/// (`<package>.<name>`, the engine's package).
#[derive(Clone, Debug)]
pub struct ShaderType {
    id: NodeTypeId,
    vocabulary: Arc<vocabulary::NodeType>,
}

/// Each node type of `engine`'s vocabulary as a shader node type of
/// `package` (the engine's package, which registers them), in the
/// vocabulary's order. Each is named under the package
/// (`kinogaki.spectre.openpbr_surface`), as everything it declares is.
///
/// # Panics
/// For a type whose id isn't well formed (an engine id or a type name
/// that isn't a lowercase id segment): a vocabulary is code, and its
/// package's tests make its shaders.
pub fn shaders(package: PackageId, engine: &EngineDescription) -> Vec<ShaderType> {
    (engine.material_nodes.iter())
        .map(|v| ShaderType::new(package, v))
        .collect()
}

/// `engine`'s material presets as creations of `package` (the engine's,
/// which registers its [`shaders`] too), in the engine's order: each
/// makes a [`MATERIAL`] named after it whose inside starts with its
/// surface, displayed (what the Material is), its settings and inputs
/// set. Each is listed in the engine's
/// family and named `<package>.preset_<name>` (its name in lowercase,
/// words joined by `_`).
///
/// # Panics
/// For a preset whose surface the engine's vocabulary lacks: presets are
/// code, and its package's tests make them.
pub fn presets(package: PackageId, engine: &EngineDescription) -> Vec<NodeCreation> {
    (engine.material_presets.iter())
        .map(|preset| {
            let surface = (engine.material_nodes.iter())
                .find(|v| v.name == preset.surface)
                .unwrap_or_else(|| panic!("{} has no surface {}", engine.id, preset.surface));
            let words: Vec<String> = (preset.name.split(|c: char| !c.is_ascii_alphanumeric()))
                .filter(|w| !w.is_empty())
                .map(str::to_ascii_lowercase)
                .collect();
            let id = format!("{}.preset_{}", package.as_str(), words.join("_"));
            NodeCreation::new(
                id,
                &*preset.name,
                NewNode::new(NodeTypeId::declared(MATERIAL)),
            )
            .family(&*engine.label)
            .aliases(preset.aliases.iter().map(|a| a.to_string()))
            .inside(ShaderType::new(package, surface).preset(preset).displayed())
        })
        .collect()
}

/// `network`, a material of `engine`, as the Shading network a Material
/// starts with: a node of `package`'s shader type for each of its nodes,
/// its settings and set constants as a preset's are, a wire for each link,
/// and the node no link leaves (the surface) displayed. How a Material
/// adopts an imported preview surface ([`EngineDescription`]'s
/// `import_preview_surface`).
///
/// # Errors
/// A node of a type the engine's vocabulary lacks, or a link naming a node
/// past the end.
pub fn seed_of(
    package: PackageId,
    engine: &EngineDescription,
    network: &MaterialNetwork,
) -> Result<NetworkSeed, Error> {
    let mut seed = NetworkSeed::default();
    for node in &network.nodes {
        let vocabulary = (engine.material_nodes.iter())
            .find(|v| v.name == node.kind)
            .ok_or_else(|| {
                Error::invalid(format_args!("{} has no node type {}", engine.id, node.kind))
            })?;
        let values = (node.values.iter()).map(|(n, c)| (&**n, c.triple().map(f64::from)));
        seed.node(ShaderType::new(package, vocabulary).node(&node.params, values));
    }
    let count = network.nodes.len();
    for link in &network.links {
        let (from, to) = (link.from as usize, link.to as usize);
        if from >= count || to >= count {
            return Err(Error::invalid("a link names a node past the end"));
        }
        seed.wire(from, &link.output, to, &link.input);
    }
    let leaves = (0..count).filter(|&n| !network.links.iter().any(|l| l.from as usize == n));
    if let [surface] = leaves.collect::<Vec<_>>()[..] {
        seed.nodes[surface].display = true;
    }
    Ok(seed)
}

impl ShaderType {
    /// `vocabulary` as a shader node type of `package`.
    ///
    /// # Panics
    /// When its id isn't well formed (see [`shaders`]).
    pub fn new(package: PackageId, vocabulary: &Arc<vocabulary::NodeType>) -> Self {
        let id = format!("{}.{}", package.as_str(), vocabulary.name);
        Self {
            id: NodeTypeId::declared(&id),
            vocabulary: vocabulary.clone(),
        }
    }

    /// The engine's own node type: its layout, defaults and sockets.
    pub fn vocabulary(&self) -> &Arc<vocabulary::NodeType> {
        &self.vocabulary
    }

    /// The material type it is the surface (a material's terminal) of
    /// (`OpenPBR`); `None` for every other node.
    pub fn surface(&self) -> Option<&str> {
        self.vocabulary.surface.as_deref()
    }

    /// The settings a node with `parameters` holds, fitted to the type
    /// (a setting it doesn't hold, or holds a value of that doesn't fit,
    /// takes its default).
    pub fn settings(&self, parameters: &Parameters) -> Params {
        let t = &self.vocabulary;
        let mut current = t.fit(&t.defaults);
        // A setting can change which others there are, and a choice's
        // index reads against its layout's choices: read until it stands.
        for _ in 0..4 {
            let read = read_settings(&t.layout(&current), parameters, &current);
            let next = t.fit(&read);
            if next == current {
                break;
            }
            current = next;
        }
        current
    }

    /// Its settings and sockets for a node with `parameters`.
    pub fn layout(&self, parameters: &Parameters) -> NodeLayout {
        self.vocabulary.layout(&self.settings(parameters))
    }

    /// The constant unwired input `socket` reads in a node with
    /// `parameters` (a number spread to three, a toggle 1 or 0); `None`
    /// for an input that isn't edited
    /// ([`InputEdit::Implicit`]) or a value of the wrong kind.
    pub fn constant(socket: &SocketSpec, parameters: &Parameters) -> Option<[f64; 3]> {
        constant_kind(&socket.edit)?;
        match parameters.get(&*socket.name)? {
            Value::Number(v) => Some([*v; 3]),
            Value::Numbers(v) => v.get(..3)?.try_into().ok(),
            Value::Bool(on) => Some([f64::from(u8::from(*on)); 3]),
            _ => None,
        }
    }

    /// A node of this type with `preset`'s settings and inputs set.
    fn preset(&self, preset: &MaterialPreset) -> NewNode {
        let values = (preset.values.iter()).map(|(n, v)| (&**n, *v));
        self.node(&preset.params, values)
    }

    /// A node of this type with settings `params` and the constants
    /// `values` (by socket) of its edited inputs: what presets and
    /// adopted materials make.
    fn node<'a>(
        &self,
        params: &Params,
        values: impl Iterator<Item = (&'a str, [f64; 3])>,
    ) -> NewNode {
        let t = &self.vocabulary;
        let settings = t.fit(params);
        let layout = t.layout(&settings);
        let mut node = NewNode::new(self.id);
        for spec in &layout.params {
            if let Some(value) = param(&settings, &spec.name) {
                node = node.with(&*spec.name, encode(spec, value));
            }
        }
        for (socket, v) in values {
            let edited = |s: &&SocketSpec| &*s.name == socket && constant_kind(&s.edit).is_some();
            let Some(spec) = layout.inputs.iter().find(edited) else {
                continue;
            };
            let value = match spec.edit {
                InputEdit::Number { .. } => Value::Number(v[0]),
                InputEdit::Toggle => Value::Bool(v[0] != 0.0),
                _ => Value::Numbers(v.to_vec()),
            };
            node = node.with(socket, value);
        }
        node
    }

    /// The default layout: the one for its default settings.
    fn default_layout(&self) -> NodeLayout {
        let t = &self.vocabulary;
        t.layout(&t.fit(&t.defaults))
    }

    /// The rows of `layout` that `keep` takes (by name and whether the
    /// layout shows it): its settings at `settings`' values, then its
    /// edited sockets at `constant`'s, those in no section first, then
    /// section by section; a socket `wired` says is wired disabled.
    fn rows(
        layout: &NodeLayout,
        settings: &Params,
        constant: impl Fn(&SocketSpec) -> Value,
        keep: impl Fn(&str, bool) -> bool,
        wired: impl Fn(usize) -> bool,
    ) -> Schema {
        let mut schema = Schema::default();
        for spec in layout.params.iter().filter(|p| keep(&p.name, p.shown)) {
            let value = param(settings, &spec.name).unwrap_or(&spec.default);
            schema = schema.parameter(setting(spec, value));
        }
        let edited: Vec<(usize, &SocketSpec, ParameterKind)> = (layout.inputs.iter().enumerate())
            .filter(|(_, s)| keep(&s.name, s.shown))
            .filter_map(|(port, s)| Some((port, s, constant_kind(&s.edit)?)))
            .collect();
        let mut sections: Vec<Option<&str>> = vec![None];
        for (_, s, _) in &edited {
            if !sections.contains(&s.section.as_deref()) {
                sections.push(s.section.as_deref());
            }
        }
        for section in sections {
            let sockets = edited
                .iter()
                .filter(|(_, s, _)| s.section.as_deref() == section);
            if let Some(label) = section
                && sockets.clone().next().is_some()
            {
                schema = schema.section(label.to_lowercase(), label, true);
            }
            for (port, s, kind) in sockets {
                let mut row = Parameter::new(&*s.name, &*s.label, kind.clone(), constant(s));
                if let Some(tip) = &s.tooltip {
                    row = row.help(&**tip);
                }
                if wired(*port) {
                    row = row.enabled(false).help(format!("{} is wired", s.label));
                }
                schema = schema.parameter(row);
            }
        }
        schema
    }
}

/// `layout`'s settings as `parameters` hold them, those it holds no
/// readable value for as `current` holds them (else their defaults).
fn read_settings(layout: &NodeLayout, parameters: &Parameters, current: &Params) -> Params {
    (layout.params.iter())
        .map(|spec| {
            let value = (parameters.get(&*spec.name))
                .and_then(|v| decode(spec, v))
                .or_else(|| param(current, &spec.name).cloned())
                .unwrap_or_else(|| spec.default.clone());
            (spec.name.clone(), value)
        })
        .collect()
}

/// Setting `spec`'s value `value` as a parameter's: a choice its index
/// into the setting's choices, a count a number, a name or path text (a
/// decoded texture, which only engines see, an empty path).
fn encode(spec: &ParamSpec, value: &ParamValue) -> Value {
    match (&spec.kind, value) {
        (ParamKind::Choice { choices, .. }, ParamValue::Choice(name)) => {
            Value::Index(choices.iter().position(|c| c.name == *name).unwrap_or(0))
        }
        (_, ParamValue::Count(n)) => Value::Number(f64::from(*n)),
        (_, ParamValue::Attribute(text) | ParamValue::File(text)) => Value::Text(text.to_string()),
        (ParamKind::Choice { .. }, _) => Value::Index(0),
        (ParamKind::Count { min, .. }, _) => Value::Number(f64::from(*min)),
        _ => Value::Text(String::new()),
    }
}

/// Parameter value `value` as setting `spec`'s, when it is one.
fn decode(spec: &ParamSpec, value: &Value) -> Option<ParamValue> {
    let decoded = match (&spec.kind, value) {
        (ParamKind::Choice { choices, .. }, Value::Index(i)) => {
            ParamValue::Choice(choices.get(*i)?.name.clone())
        }
        (ParamKind::Count { min, max }, Value::Number(n)) if n.is_finite() => {
            ParamValue::Count(n.round().clamp(f64::from(*min), f64::from(*max)) as u32)
        }
        (ParamKind::Attribute, Value::Text(name)) => ParamValue::Attribute(Arc::from(&**name)),
        (ParamKind::File { .. }, Value::Text(path)) => ParamValue::File(Arc::from(&**path)),
        _ => return None,
    };
    Some(decoded)
}

/// Setting `spec`'s row at `value`. Settings shape the sockets, so none
/// is keyed over time.
fn setting(spec: &ParamSpec, value: &ParamValue) -> Parameter {
    let kind = match &spec.kind {
        ParamKind::Choice { choices, .. } => {
            ParameterKind::Choice(choices.iter().map(|c| c.label.to_string()).collect())
        }
        &ParamKind::Count { min, max } => ParameterKind::Integer {
            min: i64::from(min),
            max: i64::from(max),
        },
        ParamKind::Attribute => ParameterKind::Attribute,
        ParamKind::File { .. } => ParameterKind::File,
    };
    let row =
        Parameter::new(&*spec.name, &*spec.label, kind, encode(spec, value)).animatable(false);
    match &spec.tooltip {
        Some(tip) => row.help(&**tip),
        None => row,
    }
}

/// The kind of the constant an input edited as `edit` reads; `None` for
/// one not edited ([`InputEdit::Implicit`]).
fn constant_kind(edit: &InputEdit) -> Option<ParameterKind> {
    let kind = match edit {
        InputEdit::Number {
            range,
            soft,
            decimals,
            ..
        } => {
            let mut number = Number::plain()
                .range(range.0, range.1)
                .soft(soft.0, soft.1)
                .decimals(*decimals);
            if *range == (0.0, 1.0) {
                number = number.slider();
            }
            ParameterKind::Number(number)
        }
        InputEdit::Color => ParameterKind::Color,
        InputEdit::Vector => ParameterKind::Vector {
            components: 3,
            number: Number::plain(),
        },
        InputEdit::Toggle => ParameterKind::Toggle,
        InputEdit::Implicit(_) => return None,
    };
    Some(kind)
}

/// Edited socket `s`'s default constant: a number (a number socket's
/// first component), three numbers, or a toggle (on unless 0).
fn default_constant(s: &SocketSpec) -> Value {
    let d = s.default.map(f64::from);
    match s.edit {
        InputEdit::Number { .. } => Value::Number(d[0]),
        InputEdit::Toggle => Value::Bool(d[0] != 0.0),
        _ => Value::Numbers(d.to_vec()),
    }
}

/// `layout`'s ports: an edited input reads its constant while unwired,
/// an implicit one means what its text says.
fn ports(layout: &NodeLayout) -> Ports {
    let wire = shader_value();
    Ports {
        inputs: (layout.inputs.iter())
            .map(|s| {
                let port = Port::new(&*s.name, &*s.label, wire);
                match s.edit {
                    InputEdit::Implicit(_) => port.optional(),
                    _ => port.constant(),
                }
            })
            .collect(),
        outputs: (layout.outputs.iter())
            .map(|s| Port::new(&*s.name, &*s.label, wire))
            .collect(),
    }
}

impl NodeType for ShaderType {
    fn id(&self) -> NodeTypeId {
        self.id
    }

    /// Listed under its vocabulary category, colored a surface, a source
    /// (no inputs) or a modifier.
    fn description(&self) -> Description {
        let t = &self.vocabulary;
        let settings = t.fit(&t.defaults);
        let layout = t.layout(&settings);
        let rows = Self::rows(&layout, &settings, default_constant, |_, _| true, |_| false);
        let category = if t.surface.is_some() {
            Category::Surface
        } else if layout.inputs.is_empty() {
            Category::Create
        } else {
            Category::Modify
        };
        let aliases: Vec<&str> = t.aliases.iter().map(|a| &**a).collect();
        let Ports { inputs, outputs } = ports(&layout);
        let mut d = Description::new(&*t.label)
            .context(ContextId::declared(SHADING))
            .parameters(rows)
            .listed(&*t.category, category, &*t.category, &aliases);
        (d.inputs, d.outputs) = (inputs, outputs);
        d
    }

    /// Its sockets for the node's settings.
    fn ports(&self, parameters: &Parameters, _: &PortsCx) -> Option<Ports> {
        Some(ports(&self.layout(parameters)))
    }

    /// The settings and edited sockets the node's settings give it beyond
    /// its default layout's (a Switch's further inputs), at their
    /// defaults.
    fn spares(&self, parameters: &Parameters) -> Option<Schema> {
        let declared: BTreeSet<Arc<str>> = {
            let d = self.default_layout();
            let settings = d.params.iter().map(|p| p.name.clone());
            settings
                .chain(d.inputs.iter().map(|s| s.name.clone()))
                .collect()
        };
        let layout = self.layout(parameters);
        let defaults: Params = (layout.params.iter())
            .map(|p| (p.name.clone(), p.default.clone()))
            .collect();
        let spares = Self::rows(
            &layout,
            &defaults,
            default_constant,
            |name, _| !declared.contains(name),
            |_| false,
        );
        (!spares.items.is_empty()).then_some(spares)
    }

    /// The rows of the node's layout: its settings and edited sockets the
    /// layout shows (a Convert's channel only when three become one), with
    /// the current choices, a wired socket's constant disabled.
    fn schema(&self, _: Schema, parameters: &Parameters, cx: &SchemaCx<'_>) -> Schema {
        let settings = self.settings(parameters);
        let layout = self.vocabulary.layout(&settings);
        let constant = |s: &SocketSpec| {
            (parameters.get(&*s.name).cloned()).unwrap_or_else(|| default_constant(s))
        };
        Self::rows(
            &layout,
            &settings,
            constant,
            |_, shown| shown,
            |port| cx.wired(port),
        )
    }

    /// A setting edited reads against the layout its rows showed (a
    /// choice's index into the choices shown), and the settings it leaves
    /// keep their choices by name across the new layout (a Convert's
    /// channel when its From changes).
    fn edited(&self, before: &Parameters, after: Parameters, _: &SchemaCx<'_>) -> Parameters {
        let t = &self.vocabulary;
        let held = self.settings(before);
        let settings = t.fit(&read_settings(&t.layout(&held), &after, &held));
        let mut after = after;
        for spec in &t.layout(&settings).params {
            if let Some(value) = param(&settings, &spec.name) {
                after.insert(spec.name.to_string(), encode(spec, value));
            }
        }
        after
    }

    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, CookError> {
        Err(CookError::message(
            "Shading networks are compiled, not cooked node by node",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::CompileCx;
    use crate::params::SchemaItem;
    use crate::{Package, PackageId, Registry};
    use kinogaki_render_api::RenderEngine;
    use kinogaki_render_api::recording::RecordingEngine;
    use kinogaki_render_api::vocabulary::{Choice, SocketKind};
    use std::any::Any;

    /// Mix's channels, which follow its mode (as a Convert's follow its
    /// From).
    fn channels(mode: &str) -> Vec<Choice> {
        let names: &[(&str, &str)] = if mode == "mul" {
            &[("first", "X"), ("second", "Y"), ("length", "Length")]
        } else {
            &[("luminance", "Luminance"), ("first", "R"), ("second", "G")]
        };
        names.iter().map(|(n, l)| Choice::new(n, l)).collect()
    }

    /// A toy engine's vocabulary: Mix, whose count sets its inputs and
    /// whose mode its channels (shown only multiplying), and a surface.
    fn engine() -> EngineDescription {
        let spec = |name: &str, kind, default| ParamSpec {
            name: Arc::from(name),
            label: Arc::from(name),
            kind,
            default,
            tooltip: None,
            shown: true,
        };
        let mix: vocabulary::Layout = Arc::new(move |_, params: &Params| {
            let mode = param(params, "mode")
                .and_then(ParamValue::choice)
                .unwrap_or("add");
            let count = param(params, "count")
                .and_then(ParamValue::count)
                .unwrap_or(2);
            let mut channel = spec(
                "channel",
                ParamKind::Choice {
                    choices: channels(mode),
                    menu: false,
                },
                ParamValue::Choice(channels(mode)[0].name.clone()),
            );
            channel.shown = mode == "mul";
            let mut inputs = vec![SocketSpec::new(
                "factor",
                "Factor",
                SocketKind::Float,
                [0.5; 3],
            )];
            for i in 0..count {
                let mut s = SocketSpec::new(&format!("in{i}"), "In", SocketKind::Color, [0.2; 3]);
                s.section = Some(Arc::from("Inputs"));
                inputs.push(s);
            }
            let mut uv = SocketSpec::new("uv", "UV", SocketKind::Uv, [0.0; 3]);
            uv.edit = InputEdit::Implicit(Arc::from("The point's UV"));
            inputs.push(uv);
            NodeLayout {
                params: vec![
                    spec(
                        "mode",
                        ParamKind::Choice {
                            choices: vec![
                                Choice::new("add", "Add"),
                                Choice::new("mul", "Multiply"),
                            ],
                            menu: false,
                        },
                        ParamValue::Choice(Arc::from("add")),
                    ),
                    spec(
                        "count",
                        ParamKind::Count { min: 2, max: 4 },
                        ParamValue::Count(2),
                    ),
                    channel,
                ],
                inputs,
                outputs: vec![SocketSpec::new("out", "Out", SocketKind::Color, [0.0; 3])],
            }
        });
        let surface: vocabulary::Layout = Arc::new(|_, _: &Params| {
            let mut thin = SocketSpec::new("thin", "Thin", SocketKind::Float, [1.0; 3]);
            thin.edit = InputEdit::Toggle;
            NodeLayout {
                params: Vec::new(),
                inputs: vec![
                    SocketSpec::new("base_color", "Base Color", SocketKind::Color, [0.8; 3]),
                    thin,
                ],
                outputs: vec![SocketSpec::new(
                    "surface",
                    "Surface",
                    SocketKind::Color,
                    [0.0; 3],
                )],
            }
        });
        let node = |name: &str, category: &str, surface: Option<&str>, layout| {
            Arc::new(vocabulary::NodeType {
                engine: Arc::from("toy"),
                name: Arc::from(name),
                label: Arc::from(name),
                category: Arc::from(category),
                aliases: vec![Arc::from("blend")],
                surface: surface.map(Arc::from),
                defaults: Vec::new(),
                layout,
            })
        };
        let recording = RecordingEngine::new(Arc::new(|| {}));
        let mut engine = (**recording.description()).clone();
        engine.id = Arc::from("toy");
        engine.material_nodes = vec![
            node("mix", "Color", None, mix),
            node("toy_surface", "Surface", Some("Toy"), surface),
        ];
        engine
    }

    /// The toy engine's package.
    fn toy() -> PackageId {
        PackageId::declared("kinogaki.toy")
    }

    #[test]
    fn presets_make_materials_starting_with_their_surface() {
        let mut engine = engine();
        engine.label = Arc::from("Toy");
        engine.material_presets = vec![MaterialPreset {
            name: Arc::from("Red, Thin-Walled"),
            aliases: vec![Arc::from("ruby")],
            surface: Arc::from("toy_surface"),
            params: Vec::new(),
            values: vec![
                (Arc::from("base_color"), [1.0, 0.0, 0.0]),
                (Arc::from("thin"), [1.0; 3]),
                (Arc::from("no_such_socket"), [2.0; 3]),
            ],
        }];
        let [glass] = &presets(toy(), &engine)[..] else {
            panic!("one preset");
        };
        assert_eq!(glass.id, "kinogaki.toy.preset_red_thin_walled");
        assert!(crate::id::well_formed(&glass.id));
        assert_eq!(
            (glass.label.as_str(), glass.family.as_str()),
            ("Red, Thin-Walled", "Toy")
        );
        assert_eq!(glass.aliases, ["ruby"]);
        assert_eq!(glass.node, NewNode::new(NodeTypeId::declared(MATERIAL)));
        let surface = NewNode::new(NodeTypeId::declared("kinogaki.toy.toy_surface"))
            .with("base_color", Value::Numbers(vec![1.0, 0.0, 0.0]))
            .with("thin", true)
            .displayed();
        assert_eq!(glass.inside.nodes, [surface]);
    }

    #[test]
    fn an_engines_network_seeds_a_material_wired_its_surface_displayed() {
        use kinogaki_render_api::{Constant, Link, ShaderNode};
        let engine = engine();
        let network = MaterialNetwork {
            engine: Arc::from("toy"),
            nodes: vec![
                ShaderNode::new("mix")
                    .with_param("mode", ParamValue::Choice(Arc::from("mul")))
                    .with("factor", Constant::Number(0.25)),
                ShaderNode::new("toy_surface").with("thin", Constant::Number(0.0)),
            ],
            links: vec![Link {
                from: 0,
                output: Arc::from("out"),
                to: 1,
                input: Arc::from("base_color"),
            }],
        };
        let seed = seed_of(toy(), &engine, &network).unwrap();
        let mix = NewNode::new(NodeTypeId::declared("kinogaki.toy.mix"))
            .with("mode", Value::Index(1))
            .with("count", 2.0)
            .with("channel", Value::Index(0))
            .with("factor", 0.25);
        let surface = NewNode::new(NodeTypeId::declared("kinogaki.toy.toy_surface"))
            .with("thin", false)
            .displayed();
        assert_eq!(seed.nodes, [mix, surface]);
        assert_eq!(seed.wires.len(), 1);
        let w = &seed.wires[0];
        assert_eq!(
            (w.from, w.output.as_str(), w.to, w.input.as_str()),
            (0, "out", 1, "base_color")
        );
        // A node the vocabulary lacks, or a link past the end, is refused.
        let mut unknown = network.clone();
        unknown.nodes.push(ShaderNode::new("glow"));
        assert!(seed_of(toy(), &engine, &unknown).is_err());
        let mut dangling = network;
        dangling.links[0].to = 7;
        assert!(seed_of(toy(), &engine, &dangling).is_err());
    }

    fn names(schema: &Schema) -> Vec<&str> {
        schema.parameters().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn each_vocabulary_type_is_a_shading_node_type() {
        let engine = engine();
        let shaders = shaders(toy(), &engine);
        let ids: Vec<&str> = shaders.iter().map(|s| s.id().as_str()).collect();
        assert_eq!(ids, ["kinogaki.toy.mix", "kinogaki.toy.toy_surface"]);
        let [mix, surface] = &shaders[..] else {
            panic!("two shaders");
        };
        assert!(Arc::ptr_eq(mix.vocabulary(), &engine.material_nodes[0]));
        assert_eq!((mix.surface(), surface.surface()), (None, Some("Toy")));
        let d = mix.description();
        assert_eq!(d.label, "mix");
        assert_eq!(d.contexts, [ContextId::declared(SHADING)]);
        let listing = d.listing.expect("listed");
        assert_eq!(
            (
                listing.section.as_str(),
                listing.family.as_str(),
                listing.category
            ),
            ("Color", "Color", Category::Modify)
        );
        assert_eq!(listing.aliases, ["blend"]);
        let tile = surface.description().listing.unwrap().category;
        assert_eq!(tile, Category::Surface);
        // Every socket carries the one shader value; edited inputs read
        // their constants, the implicit one stays unwired.
        let inputs: Vec<(&str, bool, bool)> = (d.inputs.iter())
            .map(|p| (p.name.as_str(), p.constant, p.optional))
            .collect();
        assert_eq!(
            inputs,
            [
                ("factor", true, false),
                ("in0", true, false),
                ("in1", true, false),
                ("uv", false, true)
            ]
        );
        assert!(
            d.inputs
                .iter()
                .chain(&d.outputs)
                .all(|p| p.data == shader_value())
        );
        assert_eq!(d.outputs[0].name, "out");
    }

    #[test]
    fn settings_and_constants_are_parameters_at_their_defaults() {
        let engine = engine();
        let shaders = shaders(toy(), &engine);
        let d = shaders[0].description();
        // Settings (hidden ones too: they hold values), then the sockets
        // in no section, then each section's; the implicit socket has none.
        assert_eq!(
            names(&d.parameters),
            ["mode", "count", "channel", "factor", "in0", "in1"]
        );
        let section = d.parameters.items.iter().find_map(|i| match i {
            SchemaItem::Section(s) => Some(s.label.as_str()),
            _ => None,
        });
        assert_eq!(section, Some("Inputs"));
        let values = d.parameters.values();
        assert_eq!(values["mode"], Value::Index(0));
        assert_eq!(values["count"], Value::Number(2.0));
        assert_eq!(values["channel"], Value::Index(0));
        assert_eq!(values["factor"], Value::Number(0.5));
        assert_eq!(values["in1"], Value::Numbers(vec![f64::from(0.2f32); 3]));
        let mode = d.parameters.parameters().next().unwrap();
        assert_eq!(
            mode.kind,
            ParameterKind::Choice(vec!["Add".into(), "Multiply".into()])
        );
        assert!(!mode.animatable);
        let count = d.parameters.parameters().nth(1).unwrap();
        assert_eq!(count.kind, ParameterKind::Integer { min: 2, max: 4 });
        let ParameterKind::Number(factor) = &d.parameters.parameters().nth(3).unwrap().kind else {
            panic!("a number");
        };
        assert!(factor.slider && (factor.min, factor.max) == (0.0, 1.0));
        // A toggle is a toggle, read back as 1 or 0.
        let surface = shaders[1].description().parameters.values();
        assert_eq!(surface["thin"], Value::Bool(true));
        let layout = shaders[1].layout(&surface);
        assert_eq!(
            ShaderType::constant(&layout.inputs[1], &surface),
            Some([1.0; 3])
        );
        assert_eq!(
            ShaderType::constant(&layout.inputs[0], &surface),
            Some([f64::from(0.8f32); 3])
        );
        // The settings read back as the vocabulary's.
        let settings = shaders[0].settings(&values);
        assert_eq!(settings, engine.material_nodes[0].fit(&Vec::new()));
        let uv = &shaders[0].layout(&values).inputs[3];
        assert_eq!(ShaderType::constant(uv, &values), None);
    }

    #[test]
    fn ports_and_spares_follow_the_settings() {
        let engine = engine();
        let mix = &shaders(toy(), &engine)[0];
        let cx = PortsCx::new(ContextId::declared(SHADING));
        let mut p = mix.description().parameters.values();
        assert!(mix.spares(&p).is_none());
        p.insert("count".into(), Value::Number(4.0));
        let ports = mix.ports(&p, &cx).unwrap();
        let inputs: Vec<&str> = ports.inputs.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(inputs, ["factor", "in0", "in1", "in2", "in3", "uv"]);
        // The further inputs' constants are spares, at their defaults.
        let spares = mix.spares(&p).expect("spares");
        assert_eq!(names(&spares), ["in2", "in3"]);
        // A count past the limit is held to it.
        p.insert("count".into(), Value::Number(9.0));
        assert_eq!(mix.ports(&p, &cx).unwrap().inputs.len(), 6);
        // The rows show the channel only while multiplying, with its
        // mode's choices, and a wired input's constant disabled.
        let rows = |p: &Parameters, wired: &[bool]| {
            mix.schema(Schema::default(), p, &SchemaCx::new(wired, &[]))
        };
        let adding = rows(&p, &[]);
        assert!(!names(&adding).contains(&"channel"));
        let mut mul = p.clone();
        mul.insert("mode".into(), Value::Index(1));
        let multiplying = rows(&mul, &[false, true]);
        let channel = multiplying
            .parameters()
            .find(|r| r.name == "channel")
            .unwrap();
        assert_eq!(
            channel.kind,
            ParameterKind::Choice(vec!["X".into(), "Y".into(), "Length".into()])
        );
        let enabled: Vec<(&str, bool)> = (multiplying.parameters())
            .filter(|r| r.name.starts_with("in"))
            .map(|r| (r.name.as_str(), r.enabled))
            .collect();
        assert_eq!(
            enabled,
            [("in0", false), ("in1", true), ("in2", true), ("in3", true)]
        );
    }

    #[test]
    fn an_edited_setting_keeps_the_others_choices_by_name() {
        let engine = engine();
        let mix = &shaders(toy(), &engine)[0];
        let mut before = mix.description().parameters.values();
        before.insert("mode".into(), Value::Index(1));
        // Multiplying, channel Y ("second", index 1).
        before.insert("channel".into(), Value::Index(1));
        let mut after = before.clone();
        after.insert("mode".into(), Value::Index(0));
        let edited = mix.edited(&before, after, &SchemaCx::default());
        // Adding, "second" is G, index 2.
        assert_eq!(edited["channel"], Value::Index(2));
        let settings = mix.settings(&edited);
        assert_eq!(
            param(&settings, "channel"),
            Some(&ParamValue::Choice(Arc::from("second")))
        );
        // An edit of a constant leaves the settings as they are.
        let mut after = before.clone();
        after.insert("factor".into(), Value::Number(0.25));
        assert_eq!(
            mix.edited(&before, after.clone(), &SchemaCx::default()),
            after
        );
    }

    #[test]
    fn a_compiler_finds_its_shader_through_the_lookup() {
        let engine = engine();
        let mut package = Package::new(toy(), "Toy");
        for shader in shaders(toy(), &engine) {
            package = package.with_node_type(shader);
        }
        let mut registry = Registry::new();
        registry.register(package).unwrap();
        let cx = CompileCx::new(&[]).with_types(&registry);
        let ty = cx
            .node_type(NodeTypeId::declared("kinogaki.toy.mix"))
            .expect("registered");
        // Declared data through the trait, the vocabulary by downcasting.
        assert_eq!(ty.description().label, "mix");
        let any: &dyn Any = ty;
        let shader = any.downcast_ref::<ShaderType>().expect("a shader");
        assert_eq!(&*shader.vocabulary().name, "mix");
        assert!(Arc::ptr_eq(shader.vocabulary(), &engine.material_nodes[0]));
    }

    #[test]
    #[should_panic(expected = "malformed node type id")]
    fn a_type_named_outside_the_id_rules_is_refused() {
        let mut engine = engine();
        let mut mix = (*engine.material_nodes[0]).clone();
        mix.name = Arc::from("Mix");
        engine.material_nodes = vec![Arc::new(mix)];
        let _ = shaders(toy(), &engine);
    }

    #[test]
    fn shaders_are_named_under_the_package_registering_them() {
        let acme = PackageId::declared("acme.lux");
        let ids: Vec<&str> = (shaders(acme, &engine()).iter())
            .map(|s| s.id().as_str())
            .collect();
        assert_eq!(ids, ["acme.lux.mix", "acme.lux.toy_surface"]);
    }
}
