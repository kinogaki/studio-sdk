//! [`ImportedMaterial`]: a material as a file brings it, and
//! [`MaterialsCodec`], how a file's materials save.
use super::materials;
use crate::Error;
use crate::codec::DataCodec;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field, FieldRef};
use crate::network::{NetworkSeed, NewNode, SeedWire};
use kinogaki_value::{NodeTypeId, Parameters, Value};

/// A material a file brings: the key its polygons' `material` values name,
/// the name it takes in the scene's library, and what starts its inside:
/// nodes of whatever types the format's plugin knows, which an engine renders
/// when it declares or translates them.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedMaterial {
    /// What the file's polygons name it by.
    pub key: String,
    /// The name it takes in the library.
    pub name: String,
    /// What starts its inside.
    pub inside: NetworkSeed,
}

/// Saves a file's materials as imported: `count`, then one `material` child
/// `m<i>` each holding its `key` and `name`, its nodes (`n<k>`: `type`, a
/// `name` when it has one, `display`, and its values `valueNames`,
/// `valueKinds`, `v<j>`) and its wires (`from`, `output`, `to`, `input`). A
/// round trip is exact. Numbers, colors, toggles, choices and texts save; a
/// file's material holds no other value.
///
/// ```
/// use kinogaki_studio_api::DataCodec;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::network::{NetworkSeed, NewNode};
/// use kinogaki_studio_api::standard::{self, ImportedMaterial, MaterialsCodec};
/// use kinogaki_studio_api::NodeTypeId;
///
/// let mut inside = NetworkSeed::default();
/// inside.node(NewNode::new(NodeTypeId::declared("acme.obj.surface")).with("red", 0.8).displayed());
/// let red = ImportedMaterial { key: "red".into(), name: "Red".into(), inside };
/// let data = standard::materials().wrap(vec![red.clone()]);
/// let mut saved = MemoryElement::new("result");
/// saved.write(|w| MaterialsCodec.write(&data, None, w)).unwrap();
/// let read = saved.read(|r| MaterialsCodec.read(r, None)).unwrap();
/// assert_eq!(standard::materials().get(&read), Some(&vec![red]));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct MaterialsCodec;

impl DataCodec for MaterialsCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let list = materials()
            .get(data)
            .ok_or_else(|| Error::invalid("not imported materials"))?;
        out.set("count", Field::Int(list.len() as i64))?;
        for (at, m) in list.iter().enumerate() {
            write_material(m, &mut *out.child(&format!("m{at}"), "material")?)?;
        }
        Ok(())
    }

    fn read(&self, element: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        let count = usize::try_from(element.int("count")?).map_err(|_| {
            Error::invalid(format_args!("{}: a negative count", element.location()))
        })?;
        let list = (0..count)
            .map(|at| read_material(&*element.required_child(&format!("m{at}"))?))
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(materials().wrap(list))
    }
}

fn write_material(m: &ImportedMaterial, out: &mut dyn ElementWriter) -> Result<(), Error> {
    out.set("key", Field::Text(m.key.clone()))?;
    out.set("name", Field::Text(m.name.clone()))?;
    out.set("nodes", Field::Int(m.inside.nodes.len() as i64))?;
    for (at, node) in m.inside.nodes.iter().enumerate() {
        write_node(node, &mut *out.child(&format!("n{at}"), "node")?)?;
    }
    let wires = &m.inside.wires;
    out.set(
        "from",
        Field::U32s(wires.iter().map(|w| w.from as u32).collect()),
    )?;
    out.set(
        "output",
        Field::Texts(wires.iter().map(|w| w.output.clone()).collect()),
    )?;
    out.set(
        "to",
        Field::U32s(wires.iter().map(|w| w.to as u32).collect()),
    )?;
    out.set(
        "input",
        Field::Texts(wires.iter().map(|w| w.input.clone()).collect()),
    )
}

fn write_node(node: &NewNode, out: &mut dyn ElementWriter) -> Result<(), Error> {
    out.set("type", Field::Text(node.node_type.as_str().to_owned()))?;
    out.set("name", Field::Texts(node.name.iter().cloned().collect()))?;
    out.set("display", Field::Bool(node.display))?;
    let names = node.parameters.keys().cloned().collect();
    out.set("valueNames", Field::Texts(names))?;
    let mut kinds = Vec::with_capacity(node.parameters.len());
    for (at, value) in node.parameters.values().enumerate() {
        let (kind, field) = match value {
            Value::Number(n) => (0, Field::Float(*n)),
            Value::Integer(n) => (1, Field::Int(*n)),
            Value::Numbers(n) => (2, Field::F64s(n.clone())),
            Value::Color(c) => (3, Field::F64s(c.to_vec())),
            Value::Bool(b) => (4, Field::Bool(*b)),
            Value::Choice(key) => (5, Field::Text(key.to_string())),
            Value::Text(t) => (6, Field::Text(t.clone())),
            other => {
                return Err(Error::invalid(format_args!(
                    "a file's material holds no value like {other:?}"
                )));
            }
        };
        kinds.push(kind);
        out.set(&format!("v{at}"), field)?;
    }
    out.set("valueKinds", Field::U8s(kinds))
}

fn read_material(e: &dyn ElementReader) -> Result<ImportedMaterial, Error> {
    let count = usize::try_from(e.int("nodes")?)
        .map_err(|_| Error::invalid(format_args!("{}: a negative count", e.location())))?;
    let nodes = (0..count)
        .map(|at| read_node(&*e.required_child(&format!("n{at}"))?))
        .collect::<Result<Vec<_>, Error>>()?;
    let wrong = || Error::invalid(format_args!("{}: its wires don't line up", e.location()));
    let (from, output) = (e.u32s("from")?, e.texts("output")?);
    let (to, input) = (e.u32s("to")?, e.texts("input")?);
    if from.len() != output.len() || from.len() != to.len() || from.len() != input.len() {
        return Err(wrong());
    }
    let wires = (0..from.len())
        .map(|k| SeedWire {
            from: from[k] as usize,
            output: output[k].clone(),
            to: to[k] as usize,
            input: input[k].clone(),
        })
        .collect();
    Ok(ImportedMaterial {
        key: e.text("key")?.to_owned(),
        name: e.text("name")?.to_owned(),
        inside: NetworkSeed {
            nodes,
            wires,
            insides: Vec::new(),
        },
    })
}

fn read_node(e: &dyn ElementReader) -> Result<NewNode, Error> {
    let at = e.location();
    let wrong = |what: &str| Error::invalid(format_args!("{at}: {what}"));
    let names = e.texts("valueNames")?;
    let kinds = super::attributes::u8s(e, "valueKinds")?;
    if names.len() != kinds.len() {
        return Err(wrong("its values don't line up"));
    }
    let mut parameters = Parameters::new();
    for (k, (name, kind)) in names.iter().zip(kinds).enumerate() {
        let field = e.field(&format!("v{k}"));
        let field = field.ok_or_else(|| wrong("a value is missing"))?;
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

    #[test]
    fn every_value_and_wire_round_trips() {
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
        let full = ImportedMaterial {
            key: "steel".into(),
            name: "Steel 2".into(),
            inside,
        };
        let bare = ImportedMaterial {
            key: "plain".into(),
            name: "Plain".into(),
            inside: NetworkSeed::default(),
        };
        let list = vec![full, bare];
        let data = materials().wrap(list.clone());
        let mut saved = MemoryElement::new("result");
        saved
            .write(|w| MaterialsCodec.write(&data, None, w))
            .unwrap();
        let read = saved.read(|r| MaterialsCodec.read(r, None)).unwrap();
        assert_eq!(materials().get(&read), Some(&list));
    }
}
