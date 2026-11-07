//! [`Library`]: node definitions, the graphs implementing them and the
//! geometric properties their inputs default to, read once from MaterialX
//! documents ([`library`]: MaterialX 1.39.5's libraries and Kinogaki's).
//!
//! A definition ([`NodeDef`]) is one typed signature of a node: `image` has
//! six (`ND_image_float` to `ND_image_vector4`). An inheriting definition
//! holds its parent's inputs first, its own after (overriding one of the same
//! name). Its implementation, when it is a graph ([`Graph`]), is resolved on
//! first use and kept.
use crate::graph::{self, Graph};
use crate::types::{Type, Value};
use crate::xml::{self, Element};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};

/// MaterialX's documents [`library`] reads, in order: stdlib first, as its
/// definitions are what the others' graphs are made of. Kinogaki's
/// ([`KINOGAKI`]) follow.
const MATERIALX: &[(&str, &str)] = &[
    (
        "stdlib_defs",
        include_str!("../libraries/stdlib/stdlib_defs.mtlx"),
    ),
    (
        "stdlib_ng",
        include_str!("../libraries/stdlib/stdlib_ng.mtlx"),
    ),
    (
        "pbrlib_defs",
        include_str!("../libraries/pbrlib/pbrlib_defs.mtlx"),
    ),
    (
        "pbrlib_ng",
        include_str!("../libraries/pbrlib/pbrlib_ng.mtlx"),
    ),
    (
        "open_pbr_surface",
        include_str!("../libraries/bxdf/open_pbr_surface.mtlx"),
    ),
    (
        "standard_surface",
        include_str!("../libraries/bxdf/standard_surface.mtlx"),
    ),
    ("gltf_pbr", include_str!("../libraries/bxdf/gltf_pbr.mtlx")),
    (
        "usd_preview_surface",
        include_str!("../libraries/bxdf/usd_preview_surface.mtlx"),
    ),
    (
        "disney_principled",
        include_str!("../libraries/bxdf/disney_principled.mtlx"),
    ),
    (
        "lama_add",
        include_str!("../libraries/bxdf/lama/lama_add.mtlx"),
    ),
    (
        "lama_conductor",
        include_str!("../libraries/bxdf/lama/lama_conductor.mtlx"),
    ),
    (
        "lama_dielectric",
        include_str!("../libraries/bxdf/lama/lama_dielectric.mtlx"),
    ),
    (
        "lama_diffuse",
        include_str!("../libraries/bxdf/lama/lama_diffuse.mtlx"),
    ),
    (
        "lama_emission",
        include_str!("../libraries/bxdf/lama/lama_emission.mtlx"),
    ),
    (
        "lama_generalized_schlick",
        include_str!("../libraries/bxdf/lama/lama_generalized_schlick.mtlx"),
    ),
    (
        "lama_iridescence",
        include_str!("../libraries/bxdf/lama/lama_iridescence.mtlx"),
    ),
    (
        "lama_layer",
        include_str!("../libraries/bxdf/lama/lama_layer.mtlx"),
    ),
    (
        "lama_mix",
        include_str!("../libraries/bxdf/lama/lama_mix.mtlx"),
    ),
    (
        "lama_sheen",
        include_str!("../libraries/bxdf/lama/lama_sheen.mtlx"),
    ),
    (
        "lama_sss",
        include_str!("../libraries/bxdf/lama/lama_sss.mtlx"),
    ),
    (
        "lama_surface",
        include_str!("../libraries/bxdf/lama/lama_surface.mtlx"),
    ),
    (
        "lama_translucent",
        include_str!("../libraries/bxdf/lama/lama_translucent.mtlx"),
    ),
    (
        "open_pbr_to_standard_surface",
        include_str!("../libraries/bxdf/translation/open_pbr_to_standard_surface.mtlx"),
    ),
    (
        "standard_surface_to_gltf_pbr",
        include_str!("../libraries/bxdf/translation/standard_surface_to_gltf_pbr.mtlx"),
    ),
    (
        "standard_surface_to_open_pbr",
        include_str!("../libraries/bxdf/translation/standard_surface_to_open_pbr.mtlx"),
    ),
    (
        "standard_surface_to_usd",
        include_str!("../libraries/bxdf/translation/standard_surface_to_usd.mtlx"),
    ),
    (
        "cmlib_defs",
        include_str!("../libraries/cmlib/cmlib_defs.mtlx"),
    ),
    ("cmlib_ng", include_str!("../libraries/cmlib/cmlib_ng.mtlx")),
    (
        "nprlib_defs",
        include_str!("../libraries/nprlib/nprlib_defs.mtlx"),
    ),
    (
        "nprlib_ng",
        include_str!("../libraries/nprlib/nprlib_ng.mtlx"),
    ),
    (
        "lights_defs",
        include_str!("../libraries/lights/lights_defs.mtlx"),
    ),
];

/// Kinogaki's documents (`kinogaki/`): what MaterialX has no home for, read
/// after MaterialX's, and carried by a document using one.
pub(crate) const KINOGAKI: &[(&str, &str)] = &[
    (
        "kinogaki_defs",
        include_str!("../kinogaki/kinogaki_defs.mtlx"),
    ),
    ("kinogaki_ng", include_str!("../kinogaki/kinogaki_ng.mtlx")),
];

/// The library every MaterialX node is read against: MaterialX 1.39.5's
/// stdlib, pbrlib, bxdf (with its translation graphs), cmlib, nprlib and
/// lights, and Kinogaki's (`kinogaki_phong_surface`, glTF's minification
/// filter, unlit cutouts, translations into OpenPBR Surface). Read on first
/// use, once a process.
pub fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| Library::read(MATERIALX, KINOGAKI).expect("the vendored libraries read"))
}

/// The top-level elements of `documents` (each a name and a document's
/// text), in order.
fn top_elements(documents: &[(&str, &'static str)]) -> Result<Vec<Element>, String> {
    let mut elements = Vec::new();
    for (name, text) in documents {
        let root = xml::parse_static(text).map_err(|why| format!("{name}: {why}"))?;
        elements.extend(root.children.into_iter().filter_map(|c| match c {
            xml::Child::Element(e) => Some(e),
            _ => None,
        }));
    }
    Ok(elements)
}

/// One input or output of a definition.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    /// Its name.
    pub name: String,
    /// Its type.
    pub ty: Type,
    /// Its default (an output: none); none for a type that holds no value
    /// and for an input defaulting to a geometric property.
    pub value: Option<Value>,
    /// The geometric property it reads while unconnected (`UV0`, `Nworld`;
    /// [`Library::geomprop`]).
    pub geomprop: Option<String>,
    /// Whether it is uniform: a value, never connected.
    pub uniform: bool,
    /// The names it may hold (`clamp, periodic, mirror`); empty: any.
    pub enumeration: Vec<String>,
    /// For an enumeration of numbers, the value of each name in order.
    pub enum_values: Vec<Value>,
    /// What a UI calls it (`uiname`), when its definition says.
    pub label: Option<String>,
    /// The folder a UI lists it under (`uifolder`), when its definition says.
    pub folder: Option<String>,
    /// What it means (`doc`); empty: nothing said.
    pub doc: String,
    /// The UI's hard and soft limits (`uimin`, `uimax`, `uisoftmin`,
    /// `uisoftmax`), when its definition gives them.
    pub min: Option<Value>,
    /// See [`min`](Self::min).
    pub max: Option<Value>,
    /// See [`min`](Self::min).
    pub soft_min: Option<Value>,
    /// See [`min`](Self::min).
    pub soft_max: Option<Value>,
    /// The kind of unit its value is in (`distance`, `angle`), when it has one.
    pub unit_type: Option<String>,
    /// The unit its value is in (`nanometer`, `degree`), when its definition
    /// fixes one; a value of a unit type without one is in the scene's.
    pub unit: Option<String>,
}

impl Port {
    /// The port `e` declares.
    fn read(e: &Element) -> Port {
        let ty = Type::of(e.get("type"));
        let value = |name: &str| e.attr(name).and_then(|v| Value::parse(&ty, v));
        let list = |name: &str| -> Vec<String> {
            let text = e.get(name);
            match text.is_empty() {
                true => Vec::new(),
                false => text.split(',').map(|s| s.trim().to_owned()).collect(),
            }
        };
        let enum_values = (e.attr("enumvalues").into_iter())
            .flat_map(|values| split_values(&ty, values))
            .collect();
        Port {
            name: e.get("name").to_owned(),
            value: value("value"),
            geomprop: e.attr("defaultgeomprop").map(str::to_owned),
            uniform: e.get("uniform") == "true",
            enumeration: list("enum"),
            enum_values,
            label: e.attr("uiname").map(str::to_owned),
            folder: e.attr("uifolder").map(str::to_owned),
            doc: e.get("doc").to_owned(),
            min: value("uimin"),
            max: value("uimax"),
            soft_min: value("uisoftmin"),
            soft_max: value("uisoftmax"),
            unit_type: e.attr("unittype").map(str::to_owned),
            unit: e.attr("unit").map(str::to_owned),
            ty,
        }
    }
}

/// `text`, an enumeration's values, as values of `ty`: each holds
/// `ty`'s width of the numbers (a vector2 enumeration lists pairs).
fn split_values(ty: &Type, text: &str) -> Vec<Value> {
    let parts: Vec<&str> = text.split(',').collect();
    let width = ty.width().max(1);
    (parts.chunks(width))
        .filter_map(|chunk| Value::parse(ty, &chunk.join(",")))
        .collect()
}

/// A node definition: one typed signature of a node.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeDef {
    /// Its name, the signature (`ND_image_color3`).
    pub name: String,
    /// The node it defines (`image`).
    pub node: String,
    /// The group a UI lists the node under (`texture2d`, `pbr`).
    pub group: String,
    /// Its version (`1.1.1`), when it has one.
    pub version: Option<String>,
    /// Whether it is the node's default version.
    pub default_version: bool,
    /// What a UI calls the node (`uiname`), when its definition says.
    pub label: Option<String>,
    /// What it does (`doc`); empty: nothing said.
    pub doc: String,
    /// Its inputs, its parent's first when it inherits.
    pub inputs: Vec<Port>,
    /// Its outputs.
    pub outputs: Vec<Port>,
}

impl NodeDef {
    /// The definition nodedef element `e` declares, its own inputs and
    /// outputs alone (an inheriting one's parent's aren't read here).
    pub(crate) fn of(e: &Element) -> NodeDef {
        NodeDef {
            name: e.get("name").to_owned(),
            node: e.get("node").to_owned(),
            group: e.get("nodegroup").to_owned(),
            version: e.attr("version").map(str::to_owned),
            default_version: e.get("isdefaultversion") == "true",
            label: e.attr("uiname").map(str::to_owned),
            doc: e.get("doc").to_owned(),
            inputs: (e.elements().filter(|c| c.category == "input"))
                .map(Port::read)
                .collect(),
            outputs: (e.elements().filter(|c| c.category == "output"))
                .map(Port::read)
                .collect(),
        }
    }

    /// Its input `name`.
    pub fn input(&self, name: &str) -> Option<&Port> {
        self.inputs.iter().find(|p| p.name == name)
    }

    /// Its output `name`.
    pub fn output(&self, name: &str) -> Option<&Port> {
        self.outputs.iter().find(|p| p.name == name)
    }

    /// Its type: its one output's, `multioutput` for several.
    pub fn ty(&self) -> Type {
        match &self.outputs[..] {
            [one] => one.ty.clone(),
            _ => Type::of("multioutput"),
        }
    }
}

/// A geometric property an input reads while unconnected (`geompropdef`):
/// the node reading it (`texcoord`, `normal`), and that node's space and
/// index where it takes them.
#[derive(Clone, Debug, PartialEq)]
pub struct GeomProp {
    /// The node that reads it.
    pub node: String,
    /// Its type.
    pub ty: Type,
    /// The space it is read in (`world`, `object`), when the node takes one.
    pub space: Option<String>,
    /// The index it is read at (a texture coordinate set), when the node
    /// takes one.
    pub index: Option<i64>,
}

/// Definitions, their implementations and the geometric properties (see the
/// module docs).
pub struct Library {
    defs: Vec<Arc<NodeDef>>,
    by_name: HashMap<String, usize>,
    by_node: BTreeMap<String, Vec<usize>>,
    /// Each definition's parent, the one it inherits.
    parents: Vec<Option<usize>>,
    /// Whether each definition is Kinogaki's.
    kinogaki: Vec<bool>,
    /// The documents' elements, which graphs are read from.
    elements: Vec<Element>,
    /// Each definition's graph element, by its index in `elements`, when a
    /// graph implements it.
    sources: Vec<Option<usize>>,
    /// Each definition's graph, resolved on first use.
    graphs: Vec<OnceLock<Option<Arc<Graph>>>>,
    geomprops: HashMap<String, GeomProp>,
    /// Each unit of the unit definitions: its unit type and its scale.
    units: HashMap<String, (String, f64)>,
}

impl Library {
    /// The library MaterialX's documents `materialx` and Kinogaki's
    /// `kinogaki` (each a name and a document's text) define.
    pub(crate) fn read(
        materialx: &[(&str, &'static str)],
        kinogaki: &[(&str, &'static str)],
    ) -> Result<Library, String> {
        let mut elements = top_elements(materialx)?;
        let kinogaki_from = elements.len();
        elements.extend(top_elements(kinogaki)?);
        let mut library = Library {
            defs: Vec::new(),
            by_name: HashMap::new(),
            by_node: BTreeMap::new(),
            parents: Vec::new(),
            kinogaki: Vec::new(),
            elements: Vec::new(),
            sources: Vec::new(),
            graphs: Vec::new(),
            geomprops: HashMap::new(),
            units: HashMap::new(),
        };
        let mut graphs: HashMap<&str, usize> = HashMap::new();
        let mut implemented: HashMap<&str, &str> = HashMap::new();
        let mut parents: Vec<Option<String>> = Vec::new();
        for (at, e) in elements.iter().enumerate() {
            match &*e.category {
                "nodedef" => {
                    let def = NodeDef::of(e);
                    if library.by_name.contains_key(&def.name) {
                        return Err(format!("{} is defined twice", def.name));
                    }
                    let index = library.defs.len();
                    library.by_name.insert(def.name.clone(), index);
                    library
                        .by_node
                        .entry(def.node.clone())
                        .or_default()
                        .push(index);
                    parents.push(e.attr("inherit").map(str::to_owned));
                    library.kinogaki.push(at >= kinogaki_from);
                    library.defs.push(Arc::new(def));
                }
                "nodegraph" => {
                    graphs.insert(e.get("name"), at);
                    if let Some(def) = e.attr("nodedef") {
                        implemented.insert(def, e.get("name"));
                    }
                }
                "implementation" => {
                    if let Some(graph) = e.attr("nodegraph") {
                        implemented.insert(e.get("nodedef"), graph);
                    }
                }
                "geompropdef" => {
                    let geomprop = GeomProp {
                        node: e.get("geomprop").to_owned(),
                        ty: Type::of(e.get("type")),
                        space: e.attr("space").map(str::to_owned),
                        index: e.attr("index").and_then(|i| i.parse().ok()),
                    };
                    library.geomprops.insert(e.get("name").to_owned(), geomprop);
                }
                "unitdef" => {
                    for unit in e.elements().filter(|c| c.category == "unit") {
                        let Ok(scale) = unit.get("scale").parse::<f64>() else {
                            continue;
                        };
                        let named = (e.get("unittype").to_owned(), scale);
                        library.units.insert(unit.get("name").to_owned(), named);
                    }
                }
                _ => {}
            }
        }
        library.inherit(&parents)?;
        library.sources = (library.defs.iter())
            .map(|def| {
                let graph = implemented.get(def.name.as_str())?;
                graphs.get(graph).copied()
            })
            .collect();
        library.graphs = library.defs.iter().map(|_| OnceLock::new()).collect();
        library.elements = elements;
        Ok(library)
    }

    /// Each inheriting definition given its parent's inputs and outputs
    /// first (parents before children, however deep).
    fn inherit(&mut self, parents: &[Option<String>]) -> Result<(), String> {
        self.parents = vec![None; self.defs.len()];
        let mut done = vec![false; self.defs.len()];
        for at in 0..self.defs.len() {
            self.inherit_one(at, parents, &mut done, 0)?;
        }
        Ok(())
    }

    fn inherit_one(
        &mut self,
        at: usize,
        parents: &[Option<String>],
        done: &mut [bool],
        depth: usize,
    ) -> Result<(), String> {
        if done[at] {
            return Ok(());
        }
        done[at] = true;
        let Some(parent) = &parents[at] else {
            return Ok(());
        };
        let name = &self.defs[at].name;
        let &from = (self.by_name.get(parent))
            .ok_or_else(|| format!("{name} inherits {parent}, which isn't defined"))?;
        if depth > 16 {
            return Err(format!("{name}'s inheritance loops"));
        }
        self.inherit_one(from, parents, done, depth + 1)?;
        self.parents[at] = Some(from);
        let parent = self.defs[from].clone();
        let def = Arc::make_mut(&mut self.defs[at]);
        let mut inputs = parent.inputs.clone();
        for own in std::mem::take(&mut def.inputs) {
            match inputs.iter_mut().find(|p| p.name == own.name) {
                Some(inherited) => *inherited = own,
                None => inputs.push(own),
            }
        }
        def.inputs = inputs;
        if def.outputs.is_empty() {
            def.outputs = parent.outputs.clone();
        }
        Ok(())
    }

    /// Definition `name` (`ND_image_color3`).
    pub fn def(&self, name: &str) -> Option<&Arc<NodeDef>> {
        self.by_name.get(name).map(|&at| &self.defs[at])
    }

    /// The definition an application knowing MaterialX's libraries alone
    /// knows a node of `def` by: `def` when they hold it; for one of
    /// Kinogaki's, the MaterialX definition it inherits, its own inputs
    /// beside (`ND_kinogaki_gltf_colorimage` is `ND_gltf_colorimage` with
    /// glTF's minification filter); none for one inheriting none
    /// (`ND_kinogaki_phong_surface_surfaceshader`).
    pub fn in_materialx(&self, def: &NodeDef) -> Option<&Arc<NodeDef>> {
        let mut at = *self.by_name.get(&def.name)?;
        while self.kinogaki[at] {
            at = self.parents[at]?;
        }
        Some(&self.defs[at])
    }

    /// The definition a node of `def` holding `inputs` is: `def` when it
    /// declares every one, else the one of Kinogaki's that
    /// [`in_materialx`](Self::in_materialx) gives as `def` and declares them
    /// (a node written as MaterialX's, read back); none when no definition
    /// declares them.
    pub fn holding<'n>(
        &self,
        def: &Arc<NodeDef>,
        inputs: impl IntoIterator<Item = &'n str> + Clone,
    ) -> Option<&Arc<NodeDef>> {
        let declares = |d: &NodeDef| inputs.clone().into_iter().all(|i| d.input(i).is_some());
        let &at = self.by_name.get(&def.name)?;
        if declares(def) {
            return Some(&self.defs[at]);
        }
        (self.defs.iter().enumerate())
            .filter(|(k, d)| self.kinogaki[*k] && declares(d))
            .find(|(_, d)| self.in_materialx(d).is_some_and(|m| m.name == def.name))
            .map(|(_, d)| d)
    }

    /// Node `node`'s definitions, in the order the library declares them.
    pub fn signatures(&self, node: &str) -> impl Iterator<Item = &Arc<NodeDef>> {
        (self.by_node.get(node).into_iter().flatten()).map(|&at| &self.defs[at])
    }

    /// Every node the library defines, by name, in name order.
    pub fn nodes(&self) -> impl Iterator<Item = &str> {
        self.by_node.keys().map(String::as_str)
    }

    /// Unit `name` (`millimeter`, `degree`): its unit type (`distance`,
    /// `angle`) and its scale against the type's base unit (metres for a
    /// distance, degrees for an angle).
    pub fn unit(&self, name: &str) -> Option<(&str, f64)> {
        let (unit_type, scale) = self.units.get(name)?;
        Some((unit_type, *scale))
    }

    /// The distance unit `metres` long (`millimeter` for 0.001), when the
    /// unit definitions have one.
    pub fn distance_unit(&self, metres: f64) -> Option<&str> {
        let mut distances = self.units.iter().filter(|(_, (t, _))| t == "distance");
        let found = distances.find(|(_, (_, scale))| (scale / metres - 1.0).abs() < 1e-12);
        found.map(|(name, _)| name.as_str())
    }

    /// Geometric property `name` (`UV0`, `Nworld`).
    pub fn geomprop(&self, name: &str) -> Option<&GeomProp> {
        self.geomprops.get(name)
    }

    /// Every geometric property, by name.
    pub fn geomprops(&self) -> impl Iterator<Item = (&str, &GeomProp)> {
        self.geomprops
            .iter()
            .map(|(name, prop)| (name.as_str(), prop))
    }

    /// The graph implementing `def`, resolved on first use; none when no
    /// graph implements it (a node an engine implements itself) or it
    /// doesn't resolve.
    pub fn implementation(&self, def: &NodeDef) -> Option<Arc<Graph>> {
        let &at = self.by_name.get(&def.name)?;
        let slot = &self.graphs[at];
        slot.get_or_init(|| {
            let source = &self.elements[self.sources[at]?];
            graph::resolve(self, &self.defs[at], source)
                .ok()
                .map(Arc::new)
        })
        .clone()
    }

    /// The definition a node of `node` resolves to, as MaterialX resolves one
    /// (`Node::getNodeDef`): of its type (`ty`), taking each of `inputs` (a
    /// name and type) under the same name and type, and of `version` (none:
    /// the default version, or a definition of none); the first so in
    /// library order.
    pub(crate) fn resolve(
        &self,
        node: &str,
        ty: &str,
        inputs: &[(&str, &str)],
        version: Option<&str>,
    ) -> Option<&Arc<NodeDef>> {
        self.signatures(node).find(|def| {
            let typed = match &def.outputs[..] {
                [one] => one.ty.name() == ty,
                _ => ty == "multioutput" || ty.is_empty(),
            };
            let versioned = match version {
                Some(v) => def.version.as_deref() == Some(v),
                None => def.default_version || def.version.is_none(),
            };
            typed
                && versioned
                && (inputs.iter()).all(|(name, ty)| {
                    def.input(name)
                        .is_some_and(|p| ty.is_empty() || p.ty.name() == *ty)
                })
        })
    }

    /// The source element of `def`'s graph, when one implements it.
    #[cfg(test)]
    pub(crate) fn graph_source(&self, def: &NodeDef) -> Option<&Element> {
        Some(&self.elements[self.sources[*self.by_name.get(&def.name)?]?])
    }

    /// Every definition, in library order.
    pub(crate) fn defs(&self) -> impl Iterator<Item = &Arc<NodeDef>> {
        self.defs.iter()
    }
}

#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
