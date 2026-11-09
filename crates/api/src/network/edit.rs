//! [`NetworkEdit`]: the edits a package makes to a network.
use super::{Flag, NetworkRead, Wire};
use crate::cook::Outputs;
use crate::data::Data;
use crate::id::{CreationId, NodeTypeId};
use crate::menu::Home;
use crate::params::{Parameters, Value};
use crate::refs::NodeRef;
use crate::{Error, Key, ValueKey};
use kinogaki_geometry::Time;

/// A network a package edits through the host, inside one undo step. Each
/// call refuses what doesn't fit (an unknown node, a wire into a port the
/// node hasn't got, a value its parameter doesn't take
/// ([`Parameter::check`](crate::params::Parameter::check)), a flag its
/// context doesn't offer) and changes nothing then; the host checks the
/// whole network again when the step is committed (no cycles, required
/// values) and drops the step when that fails.
pub trait NetworkEdit: NetworkRead {
    /// Adds `node` on the level inside `subnet` (`None`: the top level),
    /// placed and wired as `place` says, named as nothing on that level
    /// is: a new node of its type with its parameters set over the type's
    /// defaults (a [real size](crate::params::Number::real_size) in the
    /// document's unit), displayed when `node.display` says so (a grown
    /// node where the growth rule puts the flag).
    fn add_node_in(
        &mut self,
        subnet: Option<NodeRef>,
        node: NewNode,
        at: Insertion,
    ) -> Result<NodeRef, Error>;

    /// Adds `node` on the top level at `position`, wired to nothing
    /// ([`add_node_in`](Self::add_node_in) with [`Insertion::At`]).
    fn add_node(&mut self, node: NewNode, position: [f32; 2]) -> Result<NodeRef, Error> {
        self.add_node_in(None, node, Insertion::At(position))
    }

    /// Renames `node` to `name`, numbered when its level has a node so
    /// named. Paths naming it follow.
    fn rename(&mut self, node: NodeRef, name: &str) -> Result<(), Error>;

    /// Moves `node`'s tile (or a subnet's) to `position`; nothing it
    /// computes changes.
    fn set_position(&mut self, node: NodeRef, position: [f32; 2]) -> Result<(), Error>;

    /// Sets parameter `name` of `node` to `value`: on a row with an
    /// animated channel, a key at the current time on every component (a
    /// new one between keys).
    fn set_parameter(&mut self, node: NodeRef, name: &str, value: Value) -> Result<(), Error>;

    /// Replaces every parameter of `node`: `parameters` hold each one its
    /// type declares. An animated row it changes is its key at the current
    /// time, as [`set_parameter`](Self::set_parameter) sets it; one it
    /// holds at its static value (as [`NodeInfo::parameters`] reads it) or
    /// at its value at the current time ([`NetworkRead::value_at`]) is
    /// unchanged, its keys as they were. So writing back what was read,
    /// one row changed, keys that row alone.
    ///
    /// [`NodeInfo::parameters`]: super::NodeInfo::parameters
    fn set_parameters(&mut self, node: NodeRef, parameters: Parameters) -> Result<(), Error>;

    /// Wires `from` into input `port` of `node`: after its wires on a
    /// multi-input port, in place of its wire on another.
    fn connect(&mut self, from: Wire, node: NodeRef, port: usize) -> Result<(), Error>;

    /// Removes wire `wire` (in wire order) of input `port` of `node`.
    fn disconnect(&mut self, node: NodeRef, port: usize, wire: usize) -> Result<(), Error>;

    /// Sets flag `flag` of `node`. Freezing goes through
    /// [`freeze`](Self::freeze), which needs the outputs to keep; turning
    /// [`Flag::Frozen`] off unfreezes.
    fn set_flag(&mut self, node: NodeRef, flag: Flag, on: bool) -> Result<(), Error>;

    /// Moves the display flag of `node`'s level to it (`None`: nothing on
    /// the top level is displayed).
    fn set_display(&mut self, node: Option<NodeRef>) -> Result<(), Error>;

    /// Deletes `node`. When its context's networks are chains
    /// ([`NetworkKind::Chain`](super::NetworkKind::Chain)), what it fed takes
    /// its first input's wire, and so does the display flag.
    fn remove(&mut self, node: NodeRef) -> Result<(), Error>;

    /// Makes `node` hold `delta` over its input: its type must be a held
    /// edit ([`NodeType::held`](crate::NodeType::held)) of `delta`'s data
    /// type. The delta applies to what its input gives.
    fn set_delta(&mut self, node: NodeRef, delta: Data) -> Result<(), Error>;
    /// Edits the network node `node` holds inside it
    /// ([`Description::inside`](crate::cook::Description::inside)), within
    /// this edit (one undo step); `Err` when its type holds none.
    fn edit_inside(
        &mut self,
        node: NodeRef,
        edit: &mut dyn FnMut(&mut dyn NetworkEdit) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let _ = (node, edit);
        Err(Error::invalid(
            "this network holds no networks inside its nodes",
        ))
    }

    /// Sets `key` on channel `component` of parameter `row` of `node`, a
    /// channel that runs as a number (a vector's X is 0, a color's G 1; a
    /// number or an integer has the one channel 0), replacing its key at
    /// that time: the channel is animated from then on, its value at each
    /// moment its keys'. Refused for a row its type doesn't let animate
    /// ([`Parameter::animatable`](crate::params::Parameter::animatable)), a
    /// channel it hasn't, a value the channel doesn't take and a row a
    /// subnet's parameter drives (a row has one driver).
    fn set_key(
        &mut self,
        node: NodeRef,
        row: &str,
        component: usize,
        key: Key,
    ) -> Result<(), Error> {
        let _ = (node, row, component, key);
        Err(Error::invalid("this network's parameters don't animate"))
    }

    /// Sets `key` on parameter `row` of `node`, a row holding whole values
    /// (a toggle, a choice, a color space, a ramp, tone curves), as
    /// [`set_key`](Self::set_key) sets a number's.
    fn set_value_key(&mut self, node: NodeRef, row: &str, key: ValueKey) -> Result<(), Error> {
        let _ = (node, row, key);
        Err(Error::invalid("this network's parameters don't animate"))
    }

    /// Removes the key at `time` of channel `component` of parameter `row`
    /// of `node`; removing its last key is
    /// [`clear_channel`](Self::clear_channel).
    fn remove_key(
        &mut self,
        node: NodeRef,
        row: &str,
        component: usize,
        time: Time,
    ) -> Result<(), Error> {
        let _ = (node, row, component, time);
        Err(Error::invalid("this network's parameters don't animate"))
    }

    /// Stops animating channel `component` of parameter `row` of `node`:
    /// it keeps the value it has at the scene's current time.
    fn clear_channel(&mut self, node: NodeRef, row: &str, component: usize) -> Result<(), Error> {
        let _ = (node, row, component);
        Err(Error::invalid("this network's parameters don't animate"))
    }

    /// Freezes `node` with `outputs`, the outputs it has now (one value
    /// per output port): nothing upstream cooks for it until it is
    /// unfrozen.
    fn freeze(&mut self, node: NodeRef, outputs: Outputs) -> Result<(), Error>;
}

/// Where [`NetworkEdit::add_node_in`] inserts a new node, and what it
/// wires.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Insertion {
    /// At `position`, wired to nothing.
    At([f32; 2]),
    /// At `position`, after `node`: the new node reads `node`'s first
    /// output into its first input (when it has inputs), and every wire
    /// that read that output reads the new node instead.
    After {
        /// The node it follows.
        node: NodeRef,
        /// Where its tile goes.
        position: [f32; 2],
    },
    /// By the host's growth rule from what the level displays, placed on
    /// the host's grid (as the viewport's tools grow a network).
    Grow(Growth),
}

/// How a node added by [`Insertion::Grow`] joins what its level displays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Growth {
    /// It reads the displayed result: inserted after the displayed node,
    /// it takes the display flag and that node's readers.
    Modify,
    /// It reads nothing of the displayed result: it starts a branch that
    /// the level's Merge gathers (the displayed Merge, else a new one
    /// after the displayed node), and the flag ends on what shows
    /// everything shown before plus the new node.
    Create,
}

/// A node to add ([`NetworkEdit::add_node_in`]).
#[derive(Clone, Debug, PartialEq)]
pub struct NewNode {
    /// Its type.
    pub node_type: NodeTypeId,
    /// Its name; `None`: its type's label, numbered as nothing on its level
    /// is named.
    pub name: Option<String>,
    /// Parameters set over its type's defaults.
    pub parameters: Parameters,
    /// Whether it takes the display flag.
    pub display: bool,
}

impl NewNode {
    /// A node of `node_type` with its defaults, not displayed.
    pub fn new(node_type: NodeTypeId) -> Self {
        Self {
            node_type,
            name: None,
            parameters: Parameters::new(),
            display: false,
        }
    }

    /// A node of MaterialX's definition `signature` (`ND_image_color3`):
    /// its node's type (`kinogaki.mtlx.image`) holding the signature in its
    /// Signature row; `None` for a signature MaterialX's library lacks.
    pub fn materialx(signature: &str) -> Option<Self> {
        use kinogaki_render_api::material::mtlx::{SIGNATURE, def};
        let def = def(signature)?;
        let signature = crate::Value::Choice(std::sync::Arc::from(signature));
        Some(Self::new(def.id).with(SIGNATURE, signature))
    }

    /// With parameter `name` set to `value`.
    pub fn with(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.parameters.insert(name.into(), value.into());
        self
    }

    /// Named `name`.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Displayed.
    pub fn displayed(mut self) -> Self {
        self.display = true;
        self
    }
}

/// A network to start a node's inside with: its nodes in order (each
/// displayed as it says, at most one), the wires between them, and the
/// insides of those that hold networks of their own (a Material in a
/// library). Studio lays the nodes out, makes and wires them in one step,
/// and refuses a wire naming a node or port that isn't there.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkSeed {
    /// The nodes; wires and insides name them by index.
    pub nodes: Vec<NewNode>,
    /// The wires.
    pub wires: Vec<SeedWire>,
    /// The nodes' own insides, by node index.
    pub insides: Vec<(usize, NetworkSeed)>,
}

/// A wire of a [`NetworkSeed`]: output `output` of node `from` into input
/// `input` of node `to`, ports by name.
#[derive(Clone, Debug, PartialEq)]
pub struct SeedWire {
    /// The source node's index.
    pub from: usize,
    /// Its output port's name.
    pub output: String,
    /// The driven node's index.
    pub to: usize,
    /// Its input port's name.
    pub input: String,
}

impl NetworkSeed {
    /// `node` added; its index.
    pub fn node(&mut self, node: NewNode) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// A wire from `from`'s output `output` into `to`'s input `input`.
    pub fn wire(&mut self, from: usize, output: &str, to: usize, input: &str) {
        self.wires.push(SeedWire {
            from,
            output: output.to_owned(),
            to,
            input: input.to_owned(),
        });
    }

    /// Node `at`'s inside started with `inside`.
    pub fn inside(&mut self, at: usize, inside: NetworkSeed) {
        self.insides.push((at, inside));
    }

    /// Whether it holds no node.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Every length its MaterialX nodes hold (an input of a
    /// [`Length`](crate::value::Unit::Length): glTF PBR's thickness, a
    /// transmission depth) times `by`: a material read from or written to
    /// a file of other units than the project's, converted as its geometry
    /// is. A length a node drives is converted where its arithmetic scales
    /// it: the factor of a glTF texture, or the value a `multiply` takes,
    /// feeding it alone through the nodes that pick a channel (`extract`,
    /// `convert`, `dot`). How many driven lengths it couldn't convert (a
    /// texture of no factor drives one).
    pub fn scale_lengths(&mut self, by: f64) -> usize {
        self.scale_lengths_by(|_, _| by)
    }

    /// Every length as [`scale_lengths`](Self::scale_lengths) converts it,
    /// input `input` of node `at` times `by(at, input)`: a file saying a
    /// unit per input (a MaterialX document's `unit`, which
    /// `format::materialx` reads).
    pub(crate) fn scale_lengths_by(&mut self, by: impl Fn(usize, &str) -> f64) -> usize {
        use kinogaki_render_api::material::mtlx::{SIGNATURE, def};
        let def_of = |node: &NewNode| match node.parameters.get(SIGNATURE) {
            Some(Value::Choice(signature)) => def(signature),
            _ => None,
        };
        for (at, node) in self.nodes.iter_mut().enumerate() {
            let Some(def) = def_of(node) else { continue };
            for (name, value) in &mut node.parameters {
                let k = by(at, name);
                if k != 1.0 && is_length(&def, name) {
                    scaled(value, k);
                }
            }
        }
        let driven: Vec<(usize, f64)> = (self.wires.iter())
            .map(|w| (w, by(w.to, &w.input)))
            .filter(|(w, k)| {
                *k != 1.0 && def_of(&self.nodes[w.to]).is_some_and(|d| is_length(&d, &w.input))
            })
            .map(|(w, k)| (w.from, k))
            .collect();
        let mut unconverted = 0;
        for (from, k) in driven {
            match self.scale_of(from) {
                Some((at, input)) => {
                    let node = &mut self.nodes[at];
                    let default = (def_of(node))
                        .and_then(|d| Some(d.find_input(&input)?.value()?.value.clone()));
                    if let Some(held) = node.parameters.get_mut(&input) {
                        scaled(held, k);
                    } else if let Some(mut held) = default {
                        scaled(&mut held, k);
                        node.parameters.insert(input, held);
                    }
                }
                None => unconverted += 1,
            }
        }
        unconverted
    }

    /// The node and input holding the constant that scales what node `at`
    /// gives, as [`scale_lengths`](Self::scale_lengths) finds it.
    fn scale_of(&self, mut at: usize) -> Option<(usize, String)> {
        use kinogaki_render_api::material::mtlx::{SIGNATURE, def};
        for _ in 0..self.nodes.len() {
            if self.wires.iter().filter(|w| w.from == at).count() != 1 {
                return None;
            }
            let Some(Value::Choice(signature)) = self.nodes[at].parameters.get(SIGNATURE) else {
                return None;
            };
            let def = def(signature)?;
            let wired = |input: &str| self.wires.iter().find(|w| w.to == at && w.input == input);
            match def.mtlx()?.node.as_str() {
                "extract" | "convert" | "dot" => at = wired("in")?.from,
                "multiply" => {
                    let constant = ["in1", "in2"].into_iter().find(|i| wired(i).is_none())?;
                    return Some((at, constant.to_owned()));
                }
                _ => return def.find_input("factor").map(|_| (at, "factor".to_owned())),
            }
        }
        None
    }
}

/// Whether input `name` of `def` holds a length in the scene's units.
fn is_length(def: &kinogaki_render_api::NodeDef, name: &str) -> bool {
    use kinogaki_render_api::material::mtlx;
    def.mtlx()
        .is_some_and(|source| mtlx::is_length(source, name))
}

/// `value`'s numbers times `by`.
fn scaled(value: &mut Value, by: f64) {
    match value {
        Value::Number(n) => *n *= by,
        Value::Numbers(n) => n.iter_mut().for_each(|n| *n *= by),
        Value::Color(c) => c.iter_mut().for_each(|n| *n *= by),
        _ => {}
    }
}

impl From<&kinogaki_render_api::MaterialNetwork> for NetworkSeed {
    /// A Material's inside making `network`: its nodes as it holds them (a
    /// MaterialX node's signature among its parameters), the one it shows
    /// displayed, and an Image node for each distinct image its nodes read
    /// ([`image_node::reading`](crate::image_files::image_node::reading)),
    /// wired into every input reading it.
    fn from(network: &kinogaki_render_api::MaterialNetwork) -> Self {
        use crate::image_files::image_node;
        use kinogaki_render_api::material::mtlx::SIGNATURE;
        let shown = network.output_index();
        let mut seed = NetworkSeed::default();
        for (at, node) in network.nodes().iter().enumerate() {
            let mut parameters = node.values().clone();
            if let Some(def) = node.def().mtlx() {
                let signature = crate::Value::Choice(std::sync::Arc::from(def.name.as_str()));
                parameters.insert(SIGNATURE.to_owned(), signature);
            }
            seed.node(NewNode {
                node_type: node.def().id,
                name: None,
                parameters,
                display: shown == Some(at as u32),
            });
        }
        for l in network.links() {
            seed.wire(l.from as usize, &l.output, l.to as usize, &l.input);
        }
        let mut images: Vec<(&kinogaki_geometry::image::ImageSource, usize)> = Vec::new();
        for (at, node) in network.nodes().iter().enumerate() {
            for (input, source) in node.images() {
                let image = match images.iter().find(|(s, _)| *s == source) {
                    Some((_, image)) => *image,
                    None => {
                        let image = seed.node(image_node::reading(source));
                        images.push((source, image));
                        image
                    }
                };
                seed.wire(image, image_node::IMAGE, at, input);
            }
        }
        seed
    }
}

impl TryFrom<&NetworkSeed> for kinogaki_render_api::MaterialNetwork {
    type Error = Error;

    /// The material a Material's inside makes, what a file format reads of a
    /// material it writes: its MaterialX nodes by their signatures with their
    /// values, wired, each Image node's source read by the inputs it is wired
    /// into ([`image_source`](crate::image_files::image_source)), the
    /// displayed node shown. Refused: a node of another type (an engine's
    /// own), an Image node reading a result, and a seed displaying nothing.
    fn try_from(seed: &NetworkSeed) -> Result<Self, Error> {
        use crate::image_files::{IMAGE_NODE, image_source};
        use kinogaki_render_api::material::mtlx::{SIGNATURE, def};
        let mut built = kinogaki_render_api::MaterialNetwork::builder();
        let mut made = Vec::with_capacity(seed.nodes.len());
        for node in &seed.nodes {
            if node.node_type == IMAGE_NODE {
                made.push(None);
                continue;
            }
            let signature = node.parameters.get(SIGNATURE).and_then(Value::choice);
            let def = signature.and_then(def).ok_or_else(|| {
                Error::invalid(format_args!("{} isn't a MaterialX node", node.node_type))
            })?;
            let mut values = node.parameters.clone();
            values.remove(SIGNATURE);
            made.push(Some(built.node(&def, &values)?));
        }
        let at = |n: usize| (made.get(n).copied()).ok_or_else(|| Error::invalid("no such node"));
        for w in &seed.wires {
            match (at(w.from)?, at(w.to)?) {
                (Some(from), Some(to)) => built.link(from, &w.output, to, &w.input)?,
                (None, Some(to)) => {
                    let source = image_source(&seed.nodes[w.from].parameters)?;
                    let source =
                        source.ok_or_else(|| Error::invalid("an Image node reads a result"))?;
                    built.image(to, &w.input, source)?;
                }
                (_, None) => return Err(Error::invalid("a wire into an Image node")),
            }
        }
        let shown = (seed.nodes.iter().position(|n| n.display)).and_then(|n| made[n]);
        built.build(shown.ok_or_else(|| Error::invalid("it shows no MaterialX node"))?)
    }
}

/// Another way Tab makes a node: a node of `node.node_type`
/// with its parameters set, and its inside started with nodes (a Material
/// of an engine's preset). A package declares it
/// ([`Package::with_creation`](crate::Package::with_creation)) for any
/// installed node type, its own or another package's. Studio lists it
/// wherever `node.node_type` is offered, at its own home, and makes it in
/// one undo step.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeCreation {
    /// Its item id, which menus, keys and shelves name it by, under its
    /// package's vendor (`kinogaki.spectre.glass`).
    pub id: CreationId,
    /// What menus call it ("Glass").
    pub label: String,
    /// Its home wherever its node type is offered (an engine's preset in
    /// its family).
    pub home: Home,
    /// More words search finds it by.
    pub aliases: Vec<String>,
    /// The node made; `name` `None` names it after `label`.
    pub node: NewNode,
    /// The network its inside starts with; the type must hold a network
    /// ([`Description::inside`](crate::cook::Description::inside)) when it
    /// isn't empty.
    pub inside: NetworkSeed,
}

impl NodeCreation {
    /// Creation `id` called `label` at `home`, making `node` with an empty
    /// inside.
    pub fn new(id: CreationId, label: impl Into<String>, home: Home, node: NewNode) -> Self {
        Self {
            id,
            label: label.into(),
            home,
            aliases: Vec::new(),
            node,
            inside: NetworkSeed::default(),
        }
    }

    /// Found by search under `aliases` too.
    pub fn aliases<S: Into<String>>(mut self, aliases: impl IntoIterator<Item = S>) -> Self {
        self.aliases = aliases.into_iter().map(Into::into).collect();
        self
    }

    /// With `node` next inside, wired to nothing.
    pub fn inside(mut self, node: NewNode) -> Self {
        self.inside.node(node);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Parameters;
    use crate::image_files::{IMAGE_NODE, image_node, image_source};
    use kinogaki_geometry::image::ImageSource;
    use kinogaki_render_api::MaterialNetwork;
    use kinogaki_render_api::material::mtlx::{SIGNATURE, def};

    /// Two textures reading one file seed one Image node wired into both.
    #[test]
    fn a_compiled_material_seeds_its_nodes_and_an_image_node_an_image() {
        let image = def("ND_image_color3").unwrap();
        let surface = def("ND_open_pbr_surface_surfaceshader").unwrap();
        let none = Parameters::new();
        let mut built = MaterialNetwork::builder();
        let a = built.node(&image, &none).unwrap();
        let b = built.node(&image, &none).unwrap();
        let s = built.node(&surface, &none).unwrap();
        built.link(a, "out", s, "base_color").unwrap();
        built.link(b, "out", s, "specular_color").unwrap();
        let wood = ImageSource::path("/t/wood.png");
        built.image(a, "file", wood.clone()).unwrap();
        built.image(b, "file", wood.clone()).unwrap();
        let seed = NetworkSeed::from(&built.build(s).unwrap());
        assert_eq!(seed.nodes.len(), 4);
        assert!(seed.nodes[2].display);
        let [file] = [&seed.nodes[3]];
        assert_eq!(file.node_type, IMAGE_NODE);
        assert_eq!(image_source(&file.parameters).unwrap(), Some(wood));
        let into: Vec<(usize, &str, usize, &str)> = (seed.wires.iter())
            .filter(|w| w.from == 3)
            .map(|w| (w.from, w.output.as_str(), w.to, w.input.as_str()))
            .collect();
        let image = image_node::IMAGE;
        assert_eq!(into, [(3, image, 0, "file"), (3, image, 1, "file")]);
        // A MaterialX node by its signature is the node the seed holds.
        let made = NewNode::materialx("ND_image_color3").unwrap();
        let signature = |n: &NewNode| (n.node_type, n.parameters.get(SIGNATURE).cloned());
        assert_eq!(signature(&made), signature(&seed.nodes[0]));
        assert_eq!(made.parameters.len(), 1, "its signature alone");
        assert!(NewNode::materialx("ND_nothing").is_none());
    }

    /// A material's lengths scale with a change of units, and nothing else
    /// does.
    #[test]
    fn a_seeds_lengths_scale_and_nothing_else() {
        let mut seed = NetworkSeed::default();
        let surface = NewNode::materialx("ND_gltf_pbr_surfaceshader")
            .unwrap()
            .with("thickness", 2.0)
            .with("attenuation_distance", 0.5)
            .with("ior", 1.5);
        seed.node(surface);
        let tiled = NewNode::materialx("ND_tiledimage_color3")
            .unwrap()
            .with("realworldtilesize", crate::Value::Numbers(vec![1.0, 2.0]))
            .with("uvtiling", crate::Value::Numbers(vec![3.0, 3.0]));
        seed.node(tiled);
        seed.node(NewNode::new(NodeTypeId::declared("acme.engine.surface")).with("depth", 4.0));
        // Lengths textures drive: a glTF texture's factor through the
        // channel it picks, a multiply's value, and a plain image's, which
        // has none to scale.
        let open_pbr = seed.node(NewNode::materialx("ND_open_pbr_surface_surfaceshader").unwrap());
        let texture = seed.node(
            NewNode::materialx("ND_gltf_image_color3_color3_1_0")
                .unwrap()
                .with("factor", crate::Value::Color([1.0, 0.5, 1.0])),
        );
        let pick = seed.node(NewNode::materialx("ND_extract_color3").unwrap());
        seed.wire(texture, "out", pick, "in");
        seed.wire(pick, "out", open_pbr, "transmission_depth");
        let image = seed.node(NewNode::materialx("ND_image_float").unwrap());
        let times = seed.node(NewNode::materialx("ND_multiply_float").unwrap());
        seed.wire(image, "out", times, "in1");
        seed.wire(times, "out", open_pbr, "subsurface_radius");
        let bare = seed.node(NewNode::materialx("ND_image_float").unwrap());
        seed.wire(bare, "out", 0, "thickness");
        assert_eq!(seed.scale_lengths(10.0), 1, "the bare image's");
        assert_eq!(
            seed.nodes[texture].parameters["factor"],
            crate::Value::Color([10.0, 5.0, 10.0])
        );
        assert_eq!(
            seed.nodes[times].parameters["in2"],
            crate::Value::Number(10.0)
        );
        let held = |at: usize, name: &str| seed.nodes[at].parameters[name].clone();
        assert_eq!(held(0, "thickness"), crate::Value::Number(20.0));
        assert_eq!(held(0, "attenuation_distance"), crate::Value::Number(5.0));
        assert_eq!(held(0, "ior"), crate::Value::Number(1.5));
        assert_eq!(
            held(1, "realworldtilesize"),
            crate::Value::Numbers(vec![10.0, 20.0])
        );
        assert_eq!(held(1, "uvtiling"), crate::Value::Numbers(vec![3.0, 3.0]));
        assert_eq!(
            held(2, "depth"),
            crate::Value::Number(4.0),
            "an engine's own"
        );
    }

    /// What a material's seed starts reads back as that material; an
    /// engine's node isn't MaterialX's.
    #[test]
    fn a_seed_makes_the_material_it_starts() {
        let mut built = MaterialNetwork::builder();
        let none = Parameters::new();
        let image = built.node(&def("ND_image_color3").unwrap(), &none).unwrap();
        let red = [("base_weight".to_owned(), crate::Value::Number(0.5))].into();
        let surface = def("ND_open_pbr_surface_surfaceshader").unwrap();
        let s = built.node(&surface, &red).unwrap();
        built.link(image, "out", s, "base_color").unwrap();
        built
            .image(image, "file", ImageSource::path("/t/wood.png"))
            .unwrap();
        let network = built.build(s).unwrap();
        let seed = NetworkSeed::from(&network);
        assert_eq!(MaterialNetwork::try_from(&seed).unwrap(), network);
        let mut engines = seed.clone();
        engines.nodes[1] = NewNode::new(NodeTypeId::declared("acme.engine.surface")).displayed();
        let refused = MaterialNetwork::try_from(&engines).unwrap_err();
        assert!(
            refused.to_string().contains("isn't a MaterialX node"),
            "{refused}"
        );
    }
}
