//! Material networks: MaterialX's nodes ([`mtlx`]) and engines' own, wired.
//! A network is nodes of declared types ([`NodeDef`]), each holding its
//! inputs' values, links between their ports by name, and the one node it
//! shows: a `surfacematerial`, or a surface. Made by
//! [`MaterialNetwork::builder`], which checks every value against its port,
//! every link's sockets, and that nothing cycles; an engine never reads a
//! half-formed network.
//!
//! A host sends an engine a network as the engine adopts it
//! ([`Engines::adopt`](crate::Engines::adopt)): the MaterialX nodes it
//! implements as they are, the others expanded into the graphs defining them
//! or a shading model translated into one it renders ([`standard`]), and what
//! is left out named ([`MaterialNetwork::cut`]).
//!
//! An empty network ([`MaterialNetwork::default`]) renders as the engine's
//! default material: what a look naming nothing shows.
//!
//! Colors are linear: authored in the working space, ACEScg in a network
//! an engine receives (the host converts them as it sends the network;
//! [`crate::RENDER_SPACE`]).
pub(crate) mod adopt;
mod def;
mod display;
pub mod mtlx;
pub mod preview;
pub mod standard;

pub use def::{Input, NodeDef, Output, Socket};
pub use display::{DisplayTexture, MaterialDisplay};

use crate::settings::Textures;
use kinogaki_geometry::Error;
use kinogaki_geometry::image::{ImageColor, ImageSource};
use kinogaki_value::{Parameters, Value, ValueKind};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// A node's index in its network.
pub type NodeIndex = u32;

/// A node: its definition, every input's value (its default when never
/// set), the images its [`Image`](Socket::Image) inputs read, and, as an
/// engine receives it, the Texture prims the host read those to, by input
/// name.
#[derive(Clone)]
pub struct Node {
    /// Its definition: its type and, for a MaterialX node, its signature.
    pub def: Arc<NodeDef>,
    /// Every input's value that isn't a wire alone, by name.
    pub values: Parameters,
    /// What its Image inputs read, by input name: the host's Image nodes
    /// wired into them, as the host compiles a material.
    pub images: BTreeMap<String, ImageSource>,
    /// Its images' Texture prims, filled by the host as it sends the network.
    pub textures: Textures,
}

impl Node {
    /// The layer of its file its Image inputs read: its `layer` value
    /// (MaterialX's `image` names one); none, or blank, reads the main layer.
    pub fn layer(&self) -> Option<&str> {
        let named = self.values.get("layer").and_then(Value::text);
        named.map(str::trim).filter(|l| !l.is_empty())
    }

    /// Whether Image input `input` reads its file's values as stored: its
    /// image says data, or the node gives no color (MaterialX converts color
    /// spaces for colors alone; a USD UV Texture's `rgb` is one, beside its
    /// channels).
    pub fn reads_data(&self, input: &str) -> bool {
        let color =
            (self.def.outputs.iter()).any(|o| matches!(o.socket, Socket::Color3 | Socket::Color4));
        !color || (self.images.get(input)).is_some_and(|i| *i.color() == ImageColor::Data)
    }
}

impl PartialEq for Node {
    /// Equal definitions ([`NodeDef::same`]), values, images and textures.
    fn eq(&self, other: &Node) -> bool {
        self.def.same(&other.def)
            && self.values == other.values
            && self.images == other.images
            && self.textures == other.textures
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let signature = self.def.mtlx().map(|d| d.name.as_str());
        (f.debug_struct("Node"))
            .field("def", &self.def.id)
            .field("signature", &signature)
            .field("values", &self.values)
            .field("images", &self.images)
            .field("textures", &self.textures)
            .finish()
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

/// A material (see the module docs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialNetwork {
    nodes: Vec<Node>,
    links: Vec<Link>,
    output: Option<NodeIndex>,
    primvars: Arc<[Arc<str>]>,
    cut: Arc<[Arc<str>]>,
}

impl MaterialNetwork {
    /// A network with no nodes yet.
    pub fn builder() -> NetworkBuilder {
        NetworkBuilder::default()
    }

    /// Its nodes; links name them by index.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Its nodes, for a host resolving their files.
    pub fn nodes_mut(&mut self) -> &mut [Node] {
        &mut self.nodes
    }

    /// Its links, in the order they were made (an engine may emit in it).
    pub fn links(&self) -> &[Link] {
        &self.links
    }

    /// The node it shows: the terminal an engine starts from. None: it is
    /// empty, and renders as the engine's default material.
    pub fn output(&self) -> Option<&Node> {
        self.nodes.get(self.output? as usize)
    }

    /// The output node's index.
    pub fn output_index(&self) -> Option<NodeIndex> {
        self.output
    }

    /// The surface it shows: the node wired into its shown Surface
    /// Material's `surfaceshader`, else the shown node itself.
    pub fn surface(&self) -> Option<NodeIndex> {
        let shown = self.output?;
        let material = self.nodes[shown as usize]
            .def
            .mtlx()
            .is_some_and(|d| d.node == "surfacematerial");
        if !material {
            return Some(shown);
        }
        let link = self
            .links
            .iter()
            .find(|l| l.to == shown && &*l.input == "surfaceshader");
        link.map(|l| l.from)
    }

    /// The primvars its values name ([`ValueKind::Attribute`]): what a host
    /// sends the shapes using it, beside the
    /// [standard ones](crate::STANDARD_PRIMVARS).
    pub fn primvars(&self) -> &Arc<[Arc<str>]> {
        &self.primvars
    }

    /// The MaterialX nodes adoption left out for the engine it was adopted
    /// for, by node (`hextiledimage`), each once: what the engine says it
    /// doesn't render. The inputs they fed hold their own values.
    pub fn cut(&self) -> &[Arc<str>] {
        &self.cut
    }

    /// The network with its shown surface as MaterialX shading model `model`
    /// (a node: `standard_surface`), through the chain of translation graphs
    /// to it ([`kinogaki_mtlx::Library::translation`]), the translation folded
    /// and every other node as it is (an engine's own too): what a file
    /// format writing one model writes of another. None when its surface
    /// translates to none.
    pub fn translated(&self, model: &str) -> Option<MaterialNetwork> {
        adopt::translated(self, model)
    }

    /// Whether `other` has the same nodes' definitions and links, so only
    /// values differ.
    pub fn same_wiring(&self, other: &MaterialNetwork) -> bool {
        self.links == other.links
            && self.output == other.output
            && self.nodes.len() == other.nodes.len()
            && (self.nodes.iter().zip(&other.nodes)).all(|(a, b)| a.def.same(&b.def))
    }

    /// The network without the nodes `out` and every link to or from them:
    /// the inputs they fed hold their own values. Without its output node it
    /// is empty.
    pub fn without(&self, out: &BTreeSet<NodeIndex>) -> MaterialNetwork {
        if out.is_empty() {
            return self.clone();
        }
        if self.output.is_some_and(|o| out.contains(&o)) {
            return MaterialNetwork::default();
        }
        let renumbered = |n: NodeIndex| n - out.range(..n).count() as NodeIndex;
        let kept = |n: &NodeIndex| !out.contains(n);
        MaterialNetwork {
            nodes: (self.nodes.iter().enumerate())
                .filter(|(at, _)| kept(&(*at as NodeIndex)))
                .map(|(_, node)| node.clone())
                .collect(),
            links: (self.links.iter())
                .filter(|l| kept(&l.from) && kept(&l.to))
                .map(|l| Link {
                    from: renumbered(l.from),
                    to: renumbered(l.to),
                    ..l.clone()
                })
                .collect(),
            output: self.output.map(renumbered),
            primvars: self.primvars.clone(),
            cut: self.cut.clone(),
        }
    }
}

/// Makes a [`MaterialNetwork`], checking it whole.
#[derive(Clone, Debug, Default)]
pub struct NetworkBuilder {
    network: MaterialNetwork,
    primvars: BTreeSet<Arc<str>>,
}

impl NetworkBuilder {
    /// A node of `def` holding `values`, each input not named at its default;
    /// its index.
    ///
    /// # Errors
    /// A value no input of `def` holds, or one its input's kind refuses.
    pub fn node(&mut self, def: &Arc<NodeDef>, values: &Parameters) -> Result<NodeIndex, Error> {
        if let Some(name) = (values.keys())
            .find(|name| !(def.inputs.iter()).any(|i| i.value().is_some() && i.name() == *name))
        {
            let label = &def.label;
            return Err(Error::invalid(format_args!("{label} has no value {name}")));
        }
        let mut held = Parameters::new();
        for descriptor in def.inputs.iter().filter_map(Input::value) {
            let value = values.get(&descriptor.name).unwrap_or(&descriptor.value);
            descriptor
                .kind
                .check(value, &descriptor.label)
                .map_err(|why| Error::invalid(why.message))?;
            if let (ValueKind::Attribute { .. }, Value::Text(names)) = (&descriptor.kind, value) {
                self.primvars
                    .extend(names.split_whitespace().map(Arc::from));
            }
            held.insert(descriptor.name.to_string(), value.clone());
        }
        self.network.nodes.push(Node {
            def: def.clone(),
            values: held,
            images: BTreeMap::new(),
            textures: Textures::new(),
        });
        Ok((self.network.nodes.len() - 1) as NodeIndex)
    }

    /// Image input `input` of node `at` reading `image`.
    ///
    /// # Errors
    /// No such node, or no Image input so named.
    pub fn image(&mut self, at: NodeIndex, input: &str, image: ImageSource) -> Result<(), Error> {
        let node = (self.network.nodes.get_mut(at as usize))
            .ok_or_else(|| Error::invalid(format_args!("no node {at} to read an image")))?;
        let takes = node.def.find_input(input).and_then(Input::socket) == Some(&Socket::Image);
        if !takes {
            let label = &node.def.label;
            return Err(Error::invalid(format_args!(
                "{label} has no image input {input}"
            )));
        }
        node.images.insert(input.to_owned(), image);
        Ok(())
    }

    /// Output `output` of node `from` driving input `input` of node `to`.
    ///
    /// # Errors
    /// No such node or port, an input already driven, or sockets that don't
    /// meet ([`Socket::drives`]).
    pub fn link(
        &mut self,
        from: NodeIndex,
        output: &str,
        to: NodeIndex,
        input: &str,
    ) -> Result<(), Error> {
        let def = |at: NodeIndex| {
            (self.network.nodes.get(at as usize))
                .map(|n| &n.def)
                .ok_or_else(|| Error::invalid(format_args!("no node {at} to link")))
        };
        let (source, driven) = (def(from)?, def(to)?);
        let out = (source.outputs.iter()).find(|o| &*o.name == output);
        let out = out.ok_or_else(|| {
            let label = &source.label;
            Error::invalid(format_args!("{label} has no output {output}"))
        })?;
        let takes = (driven.inputs.iter()).find(|i| i.name() == input);
        let takes = takes.and_then(Input::socket).ok_or_else(|| {
            let label = &driven.label;
            Error::invalid(format_args!(
                "{label} has no input {input} a wire can drive"
            ))
        })?;
        if !out.socket.drives(takes) {
            let (a, b) = (&source.label, &driven.label);
            return Err(Error::invalid(format_args!(
                "{a}'s {output} can't drive {b}'s {input}"
            )));
        }
        let held = |l: &Link| l.to == to && &*l.input == input;
        if self.network.links.iter().any(held) {
            let label = &driven.label;
            return Err(Error::invalid(format_args!(
                "{label}'s {input} is already driven"
            )));
        }
        self.network.links.push(Link {
            from,
            output: Arc::from(output),
            to,
            input: Arc::from(input),
        });
        Ok(())
    }

    /// The network, showing node `output`.
    ///
    /// # Errors
    /// No such node, or a link cycles.
    pub fn build(mut self, output: NodeIndex) -> Result<MaterialNetwork, Error> {
        if output as usize >= self.network.nodes.len() {
            return Err(Error::invalid(format_args!("no node {output} to show")));
        }
        self.check_cycles()?;
        self.network.output = Some(output);
        self.network.primvars = self.primvars.into_iter().collect();
        Ok(self.network)
    }

    /// Kahn's walk: what no walk reaches sits on a cycle.
    fn check_cycles(&self) -> Result<(), Error> {
        let count = self.network.nodes.len();
        let mut waits = vec![0usize; count];
        for link in &self.network.links {
            waits[link.to as usize] += 1;
        }
        let mut ready: Vec<usize> = (0..count).filter(|&n| waits[n] == 0).collect();
        let mut seen = 0;
        while let Some(n) = ready.pop() {
            seen += 1;
            for link in self.network.links.iter().filter(|l| l.from as usize == n) {
                waits[link.to as usize] -= 1;
                if waits[link.to as usize] == 0 {
                    ready.push(link.to as usize);
                }
            }
        }
        match seen == count {
            true => Ok(()),
            false => Err(Error::invalid("a material's links cycle")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Parameter;
    use kinogaki_value::{NodeTypeId, ValueKind};

    fn color(name: &str) -> Parameter {
        Parameter::new(name, name, ValueKind::Color, Value::Color([0.5; 3]))
    }

    /// A surface output taking a color, a texture reading an attribute, and
    /// an output node taking a surface.
    fn defs() -> [Arc<NodeDef>; 3] {
        let closure = || Socket::Closure(Arc::from("surface"));
        let surface = NodeDef::new(NodeTypeId::declared("toy.m.surface"), "Surface", "Surface")
            .input(Input::Driven(color("base"), Socket::Color3))
            .output(Output::new("out", "Out", closure()));
        let attribute = ValueKind::Attribute {
            menu: Vec::new(),
            list: false,
        };
        let read = Parameter::new("name", "Name", attribute, "Cd");
        let reader = NodeDef::new(NodeTypeId::declared("toy.m.read"), "Read", "Input")
            .input(Input::Setting(read))
            .input(Input::Driven(color("gain"), Socket::Color3))
            .output(Output::new("out", "Out", Socket::Color3));
        let output = NodeDef::new(NodeTypeId::declared("toy.m.output"), "Output", "Output")
            .input(Input::wire("surface", "Surface", closure()));
        [Arc::new(surface), Arc::new(reader), Arc::new(output)]
    }

    #[test]
    fn a_network_is_whole_once_built() {
        let [surface, read, output] = defs();
        let mut b = MaterialNetwork::builder();
        let (none, r) = (
            Parameters::new(),
            b.node(&read, &Parameters::new()).unwrap(),
        );
        let s = b.node(&surface, &none).unwrap();
        let o = b.node(&output, &none).unwrap();
        b.link(r, "out", s, "base").unwrap();
        b.link(s, "out", o, "surface").unwrap();
        let network = b.build(o).unwrap();
        assert_eq!(network.output().map(|n| n.def.id), Some(output.id));
        assert_eq!(network.primvars()[..], [Arc::from("Cd")]);
        // Every value is there, a default never asked for.
        assert_eq!(network.nodes()[1].values["base"], Value::Color([0.5; 3]));
        assert!(
            network.nodes()[2].values.is_empty(),
            "a wire alone holds none"
        );
        // Without the reader: its link goes, the surface holds its own base.
        let cut = network.without(&BTreeSet::from([r]));
        assert_eq!(cut.links().len(), 1);
        assert_eq!(
            (cut.nodes().len(), cut.links()[0].from, cut.links()[0].to),
            (2, 0, 1)
        );
        assert_eq!(cut.output_index(), Some(1));
        // Without the output: empty.
        assert_eq!(
            network.without(&BTreeSet::from([o])),
            MaterialNetwork::default()
        );
        assert!(MaterialNetwork::default().output().is_none());
    }

    #[test]
    fn a_builder_refuses_what_isnt_whole() {
        let [surface, read, output] = defs();
        let none = Parameters::new();
        let mut b = MaterialNetwork::builder();
        let wrong = Parameters::from([("base".to_owned(), Value::Bool(true))]);
        assert!(b.node(&surface, &wrong).is_err(), "a value of another kind");
        let unknown = Parameters::from([("nope".to_owned(), Value::Bool(true))]);
        assert!(b.node(&surface, &unknown).is_err(), "no such input");
        let (s, r, o) = (
            b.node(&surface, &none).unwrap(),
            b.node(&read, &none).unwrap(),
            b.node(&output, &none).unwrap(),
        );
        // A color into a closure, a closure into a color; ports that aren't.
        assert!(b.link(r, "out", o, "surface").is_err());
        assert!(b.link(s, "out", r, "gain").is_err());
        assert!(b.link(r, "nope", s, "base").is_err());
        assert!(b.link(r, "out", s, "nope").is_err());
        assert!(
            b.link(r, "out", r, "name").is_err(),
            "a setting takes no wire"
        );
        assert!(b.link(9, "out", s, "base").is_err());
        b.link(r, "out", s, "base").unwrap();
        assert!(b.link(r, "out", s, "base").is_err(), "already driven");
        assert!(b.clone().build(9).is_err(), "no such node to show");
        // A cycle.
        let mut c = MaterialNetwork::builder();
        let (a, z) = (c.node(&read, &none).unwrap(), c.node(&read, &none).unwrap());
        c.link(a, "out", z, "gain").unwrap();
        c.link(z, "out", a, "gain").unwrap();
        assert!(c.build(a).is_err());
    }

    #[test]
    fn an_image_input_reads_a_source_as_color_or_data() {
        let color = mtlx::def("ND_image_color3").unwrap();
        let float = mtlx::def("ND_image_float").unwrap();
        let usd = mtlx::def("ND_UsdUVTexture_23").unwrap();
        let mut b = MaterialNetwork::builder();
        let none = Parameters::new();
        let (c, f, u) = (
            b.node(&color, &none).unwrap(),
            b.node(&float, &none).unwrap(),
            b.node(&usd, &none).unwrap(),
        );
        let wood = ImageSource::path("wood.png");
        b.image(c, "file", wood.clone()).unwrap();
        b.image(f, "file", wood.clone()).unwrap();
        b.image(u, "file", wood.clone()).unwrap();
        assert!(b.image(c, "default", wood.clone()).is_err(), "not an image");
        assert!(b.image(9, "file", wood.clone()).is_err(), "no such node");
        let network = b.build(c).unwrap();
        let [c, f] = [&network.nodes()[0], &network.nodes()[1]];
        assert_eq!(c.images["file"], wood);
        assert!(!c.reads_data("file"), "a color reads its colors");
        assert!(f.reads_data("file"), "a float reads its values");
        let u = &network.nodes()[2];
        assert!(!u.reads_data("file"), "a USD UV Texture's rgb is a color");
        let mut data = c.clone();
        let file = kinogaki_geometry::image::ImageFile::Path(Arc::from("wood.png"));
        let as_data = ImageSource::new(file, ImageColor::Data);
        data.images.insert("file".into(), as_data);
        assert!(data.reads_data("file"), "its image says data");
    }

    #[test]
    fn sockets_meet_by_kind() {
        let closure = |n: &str| Socket::Closure(Arc::from(n));
        assert!(Socket::Float.drives(&Socket::Color3));
        assert!(Socket::Color4.drives(&Socket::Vector2));
        assert!(Socket::Image.drives(&Socket::Image));
        assert!(!Socket::Image.drives(&Socket::String));
        assert!(!Socket::Matrix33.drives(&Socket::Matrix44));
        assert!(!Socket::Float.drives(&Socket::String));
        assert!(closure("a").drives(&closure("a")));
        assert!(!closure("a").drives(&closure("b")));
        assert!(!closure("a").drives(&Socket::Float));
    }
}
