//! [`Materials`]: the materials a file brings and the images its lights
//! name ([`ImportedMaterial`], [`ImportedImage`]), and [`MaterialsCodec`],
//! how they save.
use super::{attributes, embedded_file, materials};
use crate::Error;
use crate::codec::DataCodec;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field, FieldRef};
use crate::network::{NetworkSeed, NewNode, SeedWire};
use kinogaki_geometry::image::{ImageColor, ImageFile, ImageSource};
use kinogaki_value::{NodeTypeId, Parameters, Value};
use std::sync::Arc;

/// What a file's materials item holds ([`materials`](fn@materials)): its
/// materials, and the images its lights name. Keys are one namespace: no
/// image shares a material's key.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Materials {
    /// The file's materials, each keyed by what its polygons name it by.
    pub materials: Vec<ImportedMaterial>,
    /// The images the file's lights name (a dome's), each keyed by what
    /// they name it by after [`OP`](crate::image_files::OP).
    pub images: Vec<ImportedImage>,
}

/// A material a file brings, as the file records it.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedMaterial {
    /// What the file's polygons name it by (their `material` texts).
    pub key: String,
    /// The name it takes in the library.
    pub name: String,
    /// Its whole Shading network: every node the file records, those the
    /// shown surface doesn't read included (a classic description beside a
    /// PBR one), of the types the format knows (MaterialX's, an engine's),
    /// each texture's file an Image node wired into its `file` input
    /// ([`image_node::reading`](crate::image_files::image_node::reading)).
    pub inside: NetworkSeed,
    /// Its standard form: an OpenPBR Surface network, which a host takes
    /// when it lacks a node type `inside` holds (an engine's own material,
    /// as the engine approximates it). None when `inside` is MaterialX's
    /// nodes alone, or nothing standard approximates it.
    pub standard: Option<NetworkSeed>,
    /// Whether a surface's back shows it too (the Material's Double Sided:
    /// glTF's `doubleSided`, USD's `doubleSided`); off, a realtime engine
    /// leaves the back out.
    pub double_sided: bool,
}

/// An image a file's lights name, which the host makes an Image node of.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedImage {
    /// What the lights name it by, after [`OP`](crate::image_files::OP).
    pub key: String,
    /// The name its Image node takes.
    pub name: String,
    /// What it reads: a file on disk by its path, or the file's bytes with
    /// its name (packed in the file read, or embedded on
    /// [`options::EMBED`](crate::format::options::EMBED)), and how its
    /// colors read (a file's color space, as data).
    pub source: ImageSource,
}

/// Saves a file's materials as imported: `count`, then one `material` child
/// `m<i>` each holding its `key`, `name`, `doubleSided`, its `inside` and,
/// when it has one, its `standard` (each a `network`); then `images` and
/// one `image` child `i<k>` each holding its `key`, `name` and source
/// (`file`, a path, or an `embedded` file through its type's codec, once a
/// save for equal bytes; `color`, `space`). A network holds `nodes`, one
/// `node` child `n<k>` each (`type`, a `name` when it has one, `display`,
/// its values `valueNames`, `valueKinds` and `v<j>`, and an `inside`
/// network when it holds one) and its wires (`from`, `output`, `to`,
/// `input`). A round trip is exact. Numbers, colors, toggles, choices,
/// texts and data (an Image node's embedded file) save; a file's material
/// holds no other value.
///
/// ```
/// use kinogaki_studio_api::DataCodec;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::geometry::image::ImageSource;
/// use kinogaki_studio_api::network::{NetworkSeed, NewNode};
/// use kinogaki_studio_api::standard::{self, ImportedImage, ImportedMaterial, Materials, MaterialsCodec};
/// use kinogaki_studio_api::NodeTypeId;
///
/// let mut inside = NetworkSeed::default();
/// inside.node(NewNode::new(NodeTypeId::declared("acme.obj.surface")).with("red", 0.8).displayed());
/// let red = ImportedMaterial { key: "red".into(), name: "Red".into(), inside, standard: None, double_sided: true };
/// let sky = ImportedImage { key: "sky".into(), name: "Sky".into(), source: ImageSource::path("/hdr/sky.exr") };
/// let held = Materials { materials: vec![red], images: vec![sky] };
/// let data = standard::materials().wrap(held.clone());
/// let mut saved = MemoryElement::new("result");
/// saved.write(|w| MaterialsCodec.write(&data, w)).unwrap();
/// let read = saved.read(|r| MaterialsCodec.read(r)).unwrap();
/// assert_eq!(standard::materials().get(&read), Some(&held));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct MaterialsCodec;

impl DataCodec for MaterialsCodec {
    fn write(&self, data: &Data, out: &mut dyn ElementWriter) -> Result<(), Error> {
        let held = materials()
            .get(data)
            .ok_or_else(|| Error::invalid("not imported materials"))?;
        out.set("count", Field::Int(held.materials.len() as i64))?;
        for (at, m) in held.materials.iter().enumerate() {
            write_material(m, &mut *out.child(&format!("m{at}"), "material")?)?;
        }
        out.set("images", Field::Int(held.images.len() as i64))?;
        for (at, image) in held.images.iter().enumerate() {
            write_image(image, &mut *out.child(&format!("i{at}"), "image")?)?;
        }
        Ok(())
    }

    fn read(&self, element: &dyn ElementReader) -> Result<Data, Error> {
        let materials_held = (0..count(element, "count")?)
            .map(|at| read_material(&*element.required_child(&format!("m{at}"))?))
            .collect::<Result<Vec<_>, Error>>()?;
        let images = (0..count(element, "images")?)
            .map(|at| read_image(&*element.required_child(&format!("i{at}"))?))
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(materials().wrap(Materials {
            materials: materials_held,
            images,
        }))
    }
}

/// Field `name` of `e`, a count.
fn count(e: &dyn ElementReader, name: &str) -> Result<usize, Error> {
    usize::try_from(e.int(name)?)
        .map_err(|_| Error::invalid(format_args!("{}: a negative {name}", e.location())))
}

fn write_material(m: &ImportedMaterial, out: &mut dyn ElementWriter) -> Result<(), Error> {
    out.set("key", Field::Text(m.key.clone()))?;
    out.set("name", Field::Text(m.name.clone()))?;
    out.set("doubleSided", Field::Bool(m.double_sided))?;
    write_network(&m.inside, &mut *out.child("inside", "network")?)?;
    match &m.standard {
        Some(standard) => write_network(standard, &mut *out.child("standard", "network")?),
        None => Ok(()),
    }
}

fn read_material(e: &dyn ElementReader) -> Result<ImportedMaterial, Error> {
    let standard = e.child("standard").map(|s| read_network(&*s)).transpose()?;
    Ok(ImportedMaterial {
        key: e.text("key")?.to_owned(),
        name: e.text("name")?.to_owned(),
        inside: read_network(&*e.required_child("inside")?)?,
        standard,
        double_sided: e.bool("doubleSided")?,
    })
}

fn write_image(image: &ImportedImage, out: &mut dyn ElementWriter) -> Result<(), Error> {
    out.set("key", Field::Text(image.key.clone()))?;
    out.set("name", Field::Text(image.name.clone()))?;
    match image.source.file() {
        ImageFile::Path(path) => out.set("file", Field::Text(path.to_string()))?,
        ImageFile::Embedded(file) => out.data("embedded", &embedded_file().wrap(file.clone()))?,
    }
    // As an Image node's Color holds it: Auto, Data or a space's name.
    out.set("color", Field::Text(image.source.color().key().to_owned()))
}

fn read_image(e: &dyn ElementReader) -> Result<ImportedImage, Error> {
    let wrong = |what: &str| Error::invalid(format_args!("{}: {what}", e.location()));
    let file = match (e.field("file"), e.data("embedded")) {
        (Some(FieldRef::Text(path)), None) => ImageFile::Path(Arc::from(path)),
        (None, Some(data)) => {
            let data = data?;
            let file = embedded_file()
                .get(&data)
                .ok_or_else(|| wrong("not a file"))?;
            ImageFile::Embedded(file.clone())
        }
        _ => return Err(wrong("it reads no one file")),
    };
    let color = match e.text("color")? {
        "" => return Err(wrong("its color is no Color")),
        key => ImageColor::of_key(key),
    };
    Ok(ImportedImage {
        key: e.text("key")?.to_owned(),
        name: e.text("name")?.to_owned(),
        source: ImageSource::new(file, color),
    })
}

fn write_network(seed: &NetworkSeed, out: &mut dyn ElementWriter) -> Result<(), Error> {
    out.set("nodes", Field::Int(seed.nodes.len() as i64))?;
    for (at, node) in seed.nodes.iter().enumerate() {
        let out = &mut *out.child(&format!("n{at}"), "node")?;
        write_node(node, out)?;
        let mut held = seed.insides.iter().filter(|(n, _)| *n == at);
        if let Some((_, inside)) = held.next() {
            write_network(inside, &mut *out.child("inside", "network")?)?;
        }
        if held.next().is_some() {
            return Err(Error::invalid("a node holds two insides"));
        }
    }
    if let Some((at, _)) = seed.insides.iter().find(|(at, _)| *at >= seed.nodes.len()) {
        return Err(Error::invalid(format_args!("an inside of no node {at}")));
    }
    let index = |at: usize| u32::try_from(at).map_err(|_| Error::invalid("too many nodes"));
    let wires = &seed.wires;
    let from = wires
        .iter()
        .map(|w| index(w.from))
        .collect::<Result<_, _>>()?;
    let to = wires
        .iter()
        .map(|w| index(w.to))
        .collect::<Result<_, _>>()?;
    let output = wires.iter().map(|w| w.output.clone()).collect();
    let input = wires.iter().map(|w| w.input.clone()).collect();
    out.set("from", Field::U32s(from))?;
    out.set("output", Field::Texts(output))?;
    out.set("to", Field::U32s(to))?;
    out.set("input", Field::Texts(input))
}

fn read_network(e: &dyn ElementReader) -> Result<NetworkSeed, Error> {
    let mut seed = NetworkSeed::default();
    for at in 0..count(e, "nodes")? {
        let node = &*e.required_child(&format!("n{at}"))?;
        seed.node(read_node(node)?);
        if let Some(inside) = node.child("inside") {
            seed.inside(at, read_network(&*inside)?);
        }
    }
    let wrong = || Error::invalid(format_args!("{}: its wires don't line up", e.location()));
    let (from, output) = (e.u32s("from")?, e.texts("output")?);
    let (to, input) = (e.u32s("to")?, e.texts("input")?);
    if from.len() != output.len() || from.len() != to.len() || from.len() != input.len() {
        return Err(wrong());
    }
    let nodes = seed.nodes.len();
    for k in 0..from.len() {
        let (a, b) = (from[k] as usize, to[k] as usize);
        if a >= nodes || b >= nodes {
            return Err(wrong());
        }
        seed.wires.push(SeedWire {
            from: a,
            output: output[k].clone(),
            to: b,
            input: input[k].clone(),
        });
    }
    Ok(seed)
}

/// A value's kind as `valueKinds` holds it when it is data, a child of its
/// own.
const DATA_KIND: u8 = 7;

fn write_node(node: &NewNode, out: &mut dyn ElementWriter) -> Result<(), Error> {
    out.set("type", Field::Text(node.node_type.as_str().to_owned()))?;
    out.set("name", Field::Texts(node.name.iter().cloned().collect()))?;
    out.set("display", Field::Bool(node.display))?;
    let names = node.parameters.keys().cloned().collect();
    out.set("valueNames", Field::Texts(names))?;
    let mut kinds = Vec::with_capacity(node.parameters.len());
    for (at, value) in node.parameters.values().enumerate() {
        let name = format!("v{at}");
        let (kind, field) = match value {
            Value::Number(n) => (0, Field::Float(*n)),
            Value::Integer(n) => (1, Field::Int(*n)),
            Value::Numbers(n) => (2, Field::F64s(n.clone())),
            Value::Color(c) => (3, Field::F64s(c.to_vec())),
            Value::Bool(b) => (4, Field::Bool(*b)),
            Value::Choice(key) => (5, Field::Text(key.to_string())),
            Value::Text(t) => (6, Field::Text(t.clone())),
            Value::Data(data) => {
                out.data(&name, data)?;
                kinds.push(DATA_KIND);
                continue;
            }
            other => {
                return Err(Error::invalid(format_args!(
                    "a file's material holds no value like {other:?}"
                )));
            }
        };
        kinds.push(kind);
        out.set(&name, field)?;
    }
    out.set("valueKinds", Field::U8s(kinds))
}

fn read_node(e: &dyn ElementReader) -> Result<NewNode, Error> {
    let at = e.location();
    let wrong = |what: &str| Error::invalid(format_args!("{at}: {what}"));
    let names = e.texts("valueNames")?;
    let kinds = attributes::u8s(e, "valueKinds")?;
    if names.len() != kinds.len() {
        return Err(wrong("its values don't line up"));
    }
    let mut parameters = Parameters::new();
    for (k, (name, kind)) in names.iter().zip(kinds).enumerate() {
        let held = format!("v{k}");
        if *kind == DATA_KIND {
            let data = e.data(&held).ok_or_else(|| wrong("a value is missing"))?;
            parameters.insert(name.clone(), Value::Data(data?));
            continue;
        }
        let field = e.field(&held).ok_or_else(|| wrong("a value is missing"))?;
        let value = match (kind, field) {
            (0, FieldRef::Float(n)) => Value::Number(n),
            (1, FieldRef::Int(n)) => Value::Integer(n),
            (2, FieldRef::F64s(n)) => Value::Numbers(n.to_vec()),
            (3, FieldRef::F64s(&[r, g, b])) => Value::Color([r, g, b]),
            (4, FieldRef::Bool(b)) => Value::Bool(b),
            (5, FieldRef::Text(t)) => Value::Choice(t.into()),
            (6, FieldRef::Text(t)) => Value::Text(t.to_owned()),
            _ => return Err(wrong("a value isn't of its kind")),
        };
        parameters.insert(name.clone(), value);
    }
    let name = match e.texts("name")? {
        [] => None,
        [one] => Some(one.clone()),
        _ => return Err(wrong("it has two names")),
    };
    let ty = e.text("type")?;
    let node_type = NodeTypeId::parse(ty).ok_or_else(|| wrong("its type is no id"))?;
    Ok(NewNode {
        node_type,
        name,
        parameters,
        display: e.bool("display")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::MemoryElement;
    use crate::image_files::image_node;
    use kinogaki_geometry::EmbeddedFile;

    #[test]
    fn every_value_wire_inside_standard_form_and_image_round_trips() {
        let wood = EmbeddedFile::new("wood.png", vec![0x89, b'P', b'N', b'G']);
        let embedded = ImageSource::new(ImageFile::Embedded(wood.clone()), ImageColor::Data);
        let mut inside = NetworkSeed::default();
        let image = NewNode::new(NodeTypeId::declared("acme.obj.image"))
            .named("Base")
            .with("file", "tex/steel.png")
            .with("gain", 0.5)
            .with("mode", Value::Choice("linear".into()))
            .with("flip", true)
            .with("count", Value::Integer(i64::MAX))
            .with("offset", Value::Numbers(vec![0.25, 0.5]))
            .with("tint", Value::Color([1.0, 0.5, 0.25]));
        let surface = NewNode::new(NodeTypeId::declared("acme.obj.surface")).displayed();
        let (a, b) = (inside.node(image), inside.node(surface));
        inside.wire(a, "out", b, "base");
        let file = inside.node(image_node::reading(&embedded));
        inside.wire(file, image_node::IMAGE, b, "file");
        // An unconnected node, and a node holding a network of its own.
        inside.node(NewNode::new(NodeTypeId::declared("acme.obj.classic")));
        let holder = inside.node(NewNode::new(NodeTypeId::declared("acme.obj.group")));
        let mut held = NetworkSeed::default();
        held.node(NewNode::new(NodeTypeId::declared("acme.obj.leaf")).displayed());
        inside.inside(holder, held);
        let mut standard = NetworkSeed::default();
        standard.node(NewNode::new(NodeTypeId::declared("acme.obj.open_pbr")).displayed());
        let full = ImportedMaterial {
            key: "steel".into(),
            name: "Steel 2".into(),
            inside,
            standard: Some(standard),
            double_sided: false,
        };
        let bare = ImportedMaterial {
            key: "plain".into(),
            name: "Plain".into(),
            inside: NetworkSeed::default(),
            standard: None,
            double_sided: true,
        };
        let sky = |source| ImportedImage {
            key: "sky".into(),
            name: "Sky".into(),
            source,
        };
        let space = ImageColor::Space(Arc::from("ACEScg"));
        let held = Materials {
            materials: vec![full, bare],
            images: vec![
                sky(ImageSource::path("/hdr/sky.exr")),
                sky(ImageSource::new(ImageFile::Embedded(wood), space)),
                sky(embedded),
            ],
        };
        let data = materials().wrap(held.clone());
        let mut saved = MemoryElement::new("result");
        saved.write(|w| MaterialsCodec.write(&data, w)).unwrap();
        let read = saved.read(|r| MaterialsCodec.read(r)).unwrap();
        assert_eq!(materials().get(&read), Some(&held));
    }
}
