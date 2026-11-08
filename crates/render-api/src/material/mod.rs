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

pub use def::{NodeDef, NodeInput, NodeOutput};
pub use display::{DisplayTexture, MaterialDisplay};

use crate::id::Id;
use crate::settings::Textures;
use kinogaki_geometry::image::{ImageColor, ImageSource, TextureImage};
use kinogaki_geometry::{Error, names};
use kinogaki_mtlx::Type;
use kinogaki_value::{Parameter, Parameters, Value, ValueKind};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// A node's index in its network.
pub type NodeIndex = u32;

/// A node: its definition, every input's value (its default when never
/// set), the images its Image (`filename`) inputs read, and, as an
/// engine receives it, the Texture prims the host read those to, by input
/// name. Read-only: a network's nodes change only through the network,
/// which checks what they hold ([`MaterialNetwork::set_texture`],
/// [`MaterialNetwork::map_values`]).
#[derive(Clone)]
pub struct Node {
    def: Arc<NodeDef>,
    values: Parameters,
    images: BTreeMap<String, ImageSource>,
    textures: Textures,
}

impl Node {
    /// Its definition: its type and, for a MaterialX node, its signature.
    pub fn def(&self) -> &Arc<NodeDef> {
        &self.def
    }

    /// Every input's value that isn't a wire alone, by name.
    pub fn values(&self) -> &Parameters {
        &self.values
    }

    /// What its Image inputs read, by input name: the host's Image nodes
    /// wired into them, as the host compiles a material.
    pub fn images(&self) -> &BTreeMap<String, ImageSource> {
        &self.images
    }

    /// Its Image inputs' Texture prims, by input name: what the host read
    /// their images to as it sent the network.
    pub fn textures(&self) -> &BTreeMap<String, Id<TextureImage>> {
        &self.textures
    }

    /// The layer of its file its Image inputs read: its `layer` value
    /// (MaterialX's `image` names one); none, or blank, reads the main layer.
    pub fn layer(&self) -> Option<&str> {
        let named = self.values.get("layer").and_then(Value::text);
        named.map(str::trim).filter(|l| !l.is_empty())
    }

    /// Whether Image input `input` reads its file's values as stored: its
    /// image says data, or the node gives no color
    /// ([`NodeDef::gives_color`]; a USD UV Texture's `rgb` is one, beside
    /// its channels).
    pub fn reads_data(&self, input: &str) -> bool {
        let data = (self.images.get(input)).is_some_and(|i| *i.color() == ImageColor::Data);
        !self.def.gives_color() || data
    }

    /// `Ok` when `input` is one of its Image inputs.
    fn image_input(&self, input: &str) -> Result<(), Error> {
        match self.def.find_input(input).and_then(NodeInput::socket) == Some(&Type::Filename) {
            true => Ok(()),
            false => {
                let label = &self.def.label;
                Err(Error::invalid(format_args!(
                    "{label} has no image input {input}"
                )))
            }
        }
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

    /// The primvars its nodes read: the names its attribute values hold
    /// ([`ValueKind::Attribute`]: an engine's Attribute node's, a geometric
    /// property value's, a USD Primvar Reader's), a geometric color's `Cd`
    /// (and `Alpha`, a color4's fourth), a texture coordinate set past the
    /// first (`uv2`, `uv3`…). What a host sends the shapes using it, beside
    /// the [standard ones](crate::STANDARD_PRIMVARS).
    pub fn primvars(&self) -> &Arc<[Arc<str>]> {
        &self.primvars
    }

    /// The attributes its nodes read by name and fall back from, each once:
    /// what MaterialX's geometric property values and USD Primvar Readers
    /// name ([`mtlx::ATTRIBUTES`]). Where a shape lacks one, the reader gives
    /// its default, which a host says.
    pub fn named_attributes(&self) -> BTreeSet<Arc<str>> {
        let mut named = BTreeSet::new();
        for node in &self.nodes {
            let Some(def) = node.def.mtlx() else { continue };
            for (_, input) in (mtlx::ATTRIBUTES.iter()).filter(|(n, _)| **n == *def.node) {
                if let Some(Value::Text(name)) = node.values.get(*input)
                    && !name.trim().is_empty()
                {
                    named.insert(Arc::from(name.trim()));
                }
            }
        }
        named
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

    /// Image input `input` of node `at` reading Texture prim `texture`: what
    /// a host does as it sends the network, having read the input's image.
    ///
    /// # Errors
    /// No such node, or no Image input so named.
    pub fn set_texture(
        &mut self,
        at: NodeIndex,
        input: &str,
        texture: Id<TextureImage>,
    ) -> Result<(), Error> {
        let node = (self.nodes.get_mut(at as usize))
            .ok_or_else(|| Error::invalid(format_args!("no node {at} to read a texture")))?;
        node.image_input(input)?;
        node.textures.insert(input.to_owned(), texture);
        Ok(())
    }

    /// Every node's values through `map`, given each value's input: what a
    /// host does to a network's colors as it sends them to an engine's
    /// space. Each value `map` changes is checked again against its input,
    /// so the network stays whole.
    ///
    /// # Errors
    /// A value its input's kind refuses; the network is then unchanged.
    pub fn map_values(&mut self, mut map: impl FnMut(&Parameter, &mut Value)) -> Result<(), Error> {
        let mut mapped = Vec::with_capacity(self.nodes.len());
        let mut primvars = BTreeSet::new();
        for node in &self.nodes {
            let mut values = node.values.clone();
            for input in node.def.inputs.iter().filter_map(NodeInput::value) {
                let Some(value) = values.get_mut(&input.name) else {
                    continue;
                };
                map(input, value);
                (input.kind.check(value, &input.label))
                    .map_err(|why| Error::invalid(why.message))?;
            }
            primvars.extend(primvars_read(&node.def, &values));
            mapped.push(values);
        }
        for (node, values) in self.nodes.iter_mut().zip(mapped) {
            node.values = values;
        }
        self.primvars = primvars.into_iter().collect();
        Ok(())
    }
}

/// Whether an output carrying `output` may drive an input taking `input`:
/// numbers drive numbers (an engine converts between widths, as
/// MaterialX's `convert` would; a boolean or an integer is a number of one),
/// text drives text, an image an image, a matrix one of its size, and a
/// closure its own kind alone.
fn drives(output: &Type, input: &Type) -> bool {
    let numbers = |t: &Type| match t {
        Type::Boolean | Type::Integer => 1,
        Type::Matrix33 | Type::Matrix44 | Type::String | Type::Filename | Type::Named(_) => 0,
        other => other.width(),
    };
    match (output, input) {
        (Type::Named(a), Type::Named(b)) => a == b,
        (Type::String, Type::String) | (Type::Filename, Type::Filename) => true,
        (Type::Matrix33, Type::Matrix33) | (Type::Matrix44, Type::Matrix44) => true,
        _ => numbers(output) > 0 && numbers(input) > 0,
    }
}

/// The primvars a node of `def` holding `values` reads: the names its
/// attribute inputs hold ([`ValueKind::Attribute`]: an engine's Attribute
/// node's, a geometric property value's), and what MaterialX's geometric
/// nodes read by themselves: a geometric color's `Cd` (and `Alpha`, a
/// color4's fourth), a texture coordinate set past the first
/// ([`texcoord_set`]).
pub(crate) fn primvars_read(def: &NodeDef, values: &Parameters) -> Vec<Arc<str>> {
    let mut read: Vec<Arc<str>> = Vec::new();
    for input in def.inputs.iter().filter_map(NodeInput::value) {
        if let (ValueKind::Attribute { .. }, Some(Value::Text(names))) =
            (&input.kind, values.get(&input.name))
        {
            read.extend(names.split_whitespace().map(Arc::from));
        }
    }
    match def.mtlx() {
        Some(m) if m.node == "geomcolor" => {
            read.push(Arc::from(names::CD));
            if m.ty() == Type::Color4 {
                read.push(Arc::from(names::ALPHA));
            }
        }
        Some(m) if m.node == "texcoord" => {
            let index = values.get("index").and_then(Value::integer).unwrap_or(0);
            if index > 0 {
                read.push(texcoord_set(index));
            }
        }
        _ => {}
    }
    read
}

/// The attribute MaterialX's texture coordinate set `index` reads, as
/// formats name sets: `uv` for the first, then `uv2`, `uv3`….
pub(crate) fn texcoord_set(index: i64) -> Arc<str> {
    match index {
        ..=0 => Arc::from(names::UV),
        n => Arc::from(format!("{}{}", names::UV, n + 1)),
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
        for descriptor in def.inputs.iter().filter_map(NodeInput::value) {
            let value = values.get(&descriptor.name).unwrap_or(&descriptor.value);
            descriptor
                .kind
                .check(value, &descriptor.label)
                .map_err(|why| Error::invalid(why.message))?;
            held.insert(descriptor.name.to_string(), value.clone());
        }
        self.primvars.extend(primvars_read(def, &held));
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
        node.image_input(input)?;
        node.images.insert(input.to_owned(), image);
        Ok(())
    }

    /// Output `output` of node `from` driving input `input` of node `to`.
    ///
    /// # Errors
    /// No such node or port, an input already driven, or types that don't
    /// meet: numbers drive numbers (an engine converts between widths, as
    /// MaterialX's `convert` would; a boolean or an integer is a number of
    /// one), text drives text, an image an image, a matrix one of its size,
    /// and a closure its own kind alone.
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
        let takes = takes.and_then(NodeInput::socket).ok_or_else(|| {
            let label = &driven.label;
            Error::invalid(format_args!(
                "{label} has no input {input} a wire can drive"
            ))
        })?;
        if !drives(&out.socket, takes) {
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
        let closure = || Type::Named(Arc::from("surface"));
        let surface = NodeDef::new(NodeTypeId::declared("toy.m.surface"), "Surface", "Surface")
            .input(NodeInput::Driven(color("base"), Type::Color3))
            .output(NodeOutput::new("out", "Out", closure()));
        let attribute = ValueKind::Attribute {
            menu: Vec::new(),
            list: false,
        };
        let read = Parameter::new("name", "Name", attribute, "Cd");
        let reader = NodeDef::new(NodeTypeId::declared("toy.m.read"), "Read", "Input")
            .input(NodeInput::Setting(read))
            .input(NodeInput::Driven(color("gain"), Type::Color3))
            .output(NodeOutput::new("out", "Out", Type::Color3));
        let output = NodeDef::new(NodeTypeId::declared("toy.m.output"), "Output", "Output")
            .input(NodeInput::wire("surface", "Surface", closure()));
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
        assert_eq!(network.output().map(|n| n.def().id), Some(output.id));
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
    fn a_networks_nodes_change_only_as_it_checks_them() {
        let [surface, read, output] = defs();
        let none = Parameters::new();
        let mut b = MaterialNetwork::builder();
        let r = b.node(&read, &none).unwrap();
        let s = b.node(&surface, &none).unwrap();
        let o = b.node(&output, &none).unwrap();
        b.link(r, "out", s, "base").unwrap();
        b.link(s, "out", o, "surface").unwrap();
        let mut network = b.build(o).unwrap();
        let halved = |_: &Parameter, v: &mut Value| {
            if let Value::Color(c) = v {
                *c = c.map(|x| x * 0.5);
            }
        };
        network.map_values(halved).unwrap();
        assert_eq!(network.nodes()[1].values()["base"], Value::Color([0.25; 3]));
        assert_eq!(network.nodes()[0].values()["gain"], Value::Color([0.25; 3]));
        // An attribute renamed: the primvars follow.
        let uv = |p: &Parameter, v: &mut Value| {
            if p.name == "name" {
                *v = Value::Text("uv st".into());
            }
        };
        network.map_values(uv).unwrap();
        assert_eq!(network.primvars()[..], [Arc::from("st"), Arc::from("uv")]);
        // A value its input refuses: refused whole, nothing changed.
        let before = network.clone();
        let wrong = |_: &Parameter, v: &mut Value| {
            if matches!(v, Value::Color(_)) {
                *v = Value::Bool(true);
            }
        };
        assert!(network.map_values(wrong).is_err());
        assert_eq!(network, before);
        // A texture only where an Image input is.
        let image = mtlx::def("ND_image_color3").unwrap();
        let mut b = MaterialNetwork::builder();
        let i = b.node(&image, &none).unwrap();
        let mut network = b.build(i).unwrap();
        let texture = Id::of(crate::PrimId {
            slot: 3,
            generation: 0,
        });
        network.set_texture(i, "file", texture).unwrap();
        assert_eq!(network.nodes()[0].textures()["file"], texture);
        assert!(network.set_texture(i, "default", texture).is_err());
        assert!(network.set_texture(9, "file", texture).is_err());
    }

    #[test]
    fn a_networks_primvars_are_what_its_nodes_read() {
        // A geometric property's name, a geometric color's `Cd` (and its
        // fourth's `Alpha`), a texture coordinate set past the first.
        let none = Parameters::new();
        let mut b = MaterialNetwork::builder();
        let named = Parameters::from([("geomprop".to_owned(), Value::Text("wear".into()))]);
        let property = mtlx::def("ND_geompropvalue_float").unwrap();
        let p = b.node(&property, &named).unwrap();
        let color = b
            .node(&mtlx::def("ND_geomcolor_color4").unwrap(), &none)
            .unwrap();
        let third = Parameters::from([("index".to_owned(), Value::Integer(2))]);
        let set = b
            .node(&mtlx::def("ND_texcoord_vector2").unwrap(), &third)
            .unwrap();
        let surface = mtlx::def("ND_open_pbr_surface_surfaceshader").unwrap();
        let s = b.node(&surface, &none).unwrap();
        b.link(p, "out", s, "specular_roughness").unwrap();
        b.link(color, "out", s, "base_color").unwrap();
        b.link(set, "out", s, "geometry_tangent").unwrap();
        let network = b.build(s).unwrap();
        let names: Vec<&str> = network.primvars().iter().map(|n| &**n).collect();
        assert_eq!(names, ["Alpha", "Cd", "uv3", "wear"]);
        // Of those, the property is read by name and falls back.
        let named: Vec<Arc<str>> = network.named_attributes().into_iter().collect();
        assert_eq!(named, [Arc::from("wear")]);
        // Its name is an attribute's, as a UI offers it.
        let kind = &property
            .find_input("geomprop")
            .unwrap()
            .value()
            .unwrap()
            .kind;
        assert!(matches!(kind, ValueKind::Attribute { .. }), "{kind:?}");
    }

    #[test]
    fn sockets_meet_by_kind() {
        let closure = |n: &str| Type::Named(Arc::from(n));
        assert!(drives(&Type::Float, &Type::Color3));
        assert!(drives(&Type::Color4, &Type::Vector2));
        assert!(drives(&Type::Filename, &Type::Filename));
        assert!(!drives(&Type::Filename, &Type::String));
        assert!(!drives(&Type::Matrix33, &Type::Matrix44));
        assert!(!drives(&Type::Float, &Type::String));
        assert!(drives(&closure("a"), &closure("a")));
        assert!(!drives(&closure("a"), &closure("b")));
        assert!(!drives(&closure("a"), &Type::Float));
    }
}
