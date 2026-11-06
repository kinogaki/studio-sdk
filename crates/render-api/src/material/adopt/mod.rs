//! A network as one engine renders it ([`adopt`]), each node it doesn't
//! implement rewritten, exact steps first:
//!
//! 1. **As it is**, when the engine implements the node.
//! 2. **Expanded** into the graph defining it, when the engine renders what
//!    that graph is made of (each node of it by these same steps).
//! 3. **Translated**, for a shading model: the chain of translation graphs
//!    to a model the engine renders as it is (MaterialX's and Kinogaki's,
//!    approximations), the translation's own nodes by these steps.
//! 4. **Cut**: the node removed, what it fed holding its own value, its name
//!    kept for the engine to report ([`MaterialNetwork::cut`]).
//!
//! Then what an engine would only compute again is folded away: a pass-through
//! (`dot`) is bypassed, a node of values alone becomes the value it yields
//! ([`NodeDef::evaluate`](kinogaki_mtlx::NodeDef::evaluate)), and a node
//! reading the geometric property its readers read anyway (a texture
//! coordinate into a `texcoord` input, the shading normal into a normal) goes.
mod work;

use super::{MaterialNetwork, NodeDef, NodeIndex, mtlx};
use crate::EngineDescription;
use kinogaki_mtlx::{Type, Value, library, target};
use std::collections::HashMap;
use std::sync::Arc;
use work::{Work, mtlx_values};

/// How a node reaches an engine.
#[derive(Clone)]
enum Way {
    Native,
    Expand,
    Translate(Vec<Arc<kinogaki_mtlx::NodeDef>>),
    Cut,
}

/// Each MaterialX definition's way to a renderer, found once a rewrite:
/// `native` says what it implements as it is.
struct Ways<'a> {
    native: &'a dyn Fn(&kinogaki_mtlx::NodeDef) -> bool,
    found: HashMap<String, Way>,
}

impl Ways<'_> {
    fn way(&mut self, def: &kinogaki_mtlx::NodeDef) -> Way {
        if let Some(way) = self.found.get(&def.name) {
            return way.clone();
        }
        // A definition met again while its own way is sought loops: cut.
        self.found.insert(def.name.clone(), Way::Cut);
        let way = self.seek(def);
        self.found.insert(def.name.clone(), way.clone());
        way
    }

    fn seek(&mut self, def: &kinogaki_mtlx::NodeDef) -> Way {
        // A node of values alone folds away; one reading more is cut once
        // folding is done ([`rewrite`]).
        if (self.native)(def) || def.evaluates() {
            return Way::Native;
        }
        let library = library();
        if let Some(graph) = library.implementation(def)
            && graph
                .nodes
                .iter()
                .all(|n| !matches!(self.way(&n.def), Way::Cut))
        {
            return Way::Expand;
        }
        if def.ty() != Type::of("surfaceshader") {
            return Way::Cut;
        }
        let native = self.native;
        let renders = |model: &str| {
            (library.signatures(model)).any(|d| d.ty() == Type::of("surfaceshader") && native(d))
        };
        let Some(chain) = library.translation(&def.node, renders) else {
            return Way::Cut;
        };
        match !chain.is_empty() && chain.iter().all(|step| !matches!(self.way(step), Way::Cut)) {
            true => Way::Translate(chain),
            false => Way::Cut,
        }
    }
}

/// `network` as `engine` renders it (see the module docs).
pub(crate) fn adopt(network: &MaterialNetwork, engine: &EngineDescription) -> MaterialNetwork {
    let names = &engine.capabilities().standard_nodes;
    let native =
        |def: &kinogaki_mtlx::NodeDef| names.iter().any(|n| **n == *def.node || **n == *def.name);
    rewrite(network, &native, &|def| engine.node_def(def.id).is_some())
}

/// `network` with its shown surface as an OpenPBR Surface, its translation
/// folded: the standard form of a MaterialX material. None when its surface
/// translates to none.
pub(crate) fn standard(network: &MaterialNetwork) -> Option<MaterialNetwork> {
    let surface = Type::of("surfaceshader");
    let native = |def: &kinogaki_mtlx::NodeDef| {
        def.node == super::standard::OPEN_PBR || (def.ty() != surface && def.group != "translation")
    };
    let form = rewrite(network, &native, &|_| true);
    form.output().is_some().then_some(form)
}

/// `network` with every graph-defined MaterialX node but those `keep` names
/// expanded, all the way down, and simplified: what the viewport reads a
/// standard form's textures through.
pub(crate) fn expanded(network: &MaterialNetwork, keep: &[&str]) -> MaterialNetwork {
    let native = |def: &kinogaki_mtlx::NodeDef| {
        keep.contains(&def.node.as_str()) || library().implementation(def).is_none()
    };
    rewrite(network, &native, &|_| true)
}

/// `network` rewritten by the ladder: a MaterialX node as it is where
/// `native` says, else expanded, translated or cut; an engine's own node kept
/// where `own` says, else cut. Then simplified.
fn rewrite(
    network: &MaterialNetwork,
    native: &dyn Fn(&kinogaki_mtlx::NodeDef) -> bool,
    own: &dyn Fn(&NodeDef) -> bool,
) -> MaterialNetwork {
    let mut ways = Ways {
        native,
        found: HashMap::new(),
    };
    let mut work = Work::of(network);
    let mut open: Vec<NodeIndex> = work.live();
    while let Some(at) = open.pop() {
        let Some(node) = work.node(at) else {
            continue;
        };
        let def = node.def.clone();
        let Some(source) = def.mtlx().cloned() else {
            if !own(&def) {
                work.take(at);
                work.cut.insert(Arc::from(def.id.name()));
            }
            continue;
        };
        match ways.way(&source) {
            Way::Native => {}
            Way::Expand => {
                let graph = library()
                    .implementation(&source)
                    .expect("an expanding node has its graph");
                match work.expand(at, &graph) {
                    Some(made) => open.extend(made),
                    None => cut(&mut work, at, &source),
                }
            }
            Way::Translate(chain) => {
                let mut from = at;
                for step in chain {
                    let made = (target(&step).and_then(model))
                        .zip(mtlx::def(&step.name))
                        .and_then(|(to, step)| work.translate(from, step, to));
                    let Some([translation, made]) = made else {
                        cut(&mut work, from, &source);
                        break;
                    };
                    open.push(translation);
                    from = made;
                }
                open.push(from);
            }
            Way::Cut => cut(&mut work, at, &source),
        }
    }
    simplify(&mut work);
    // What folding left of the arithmetic an engine doesn't implement reads
    // something it can't fold: cut.
    let unfolded: Vec<NodeIndex> = (work.live().into_iter())
        .filter(|&at| {
            work.node(at)
                .and_then(|n| n.def.mtlx())
                .is_some_and(|d| !native(d))
        })
        .collect();
    if !unfolded.is_empty() {
        for at in unfolded {
            if let Some(def) = work.node(at).and_then(|n| n.def.mtlx().cloned()) {
                cut(&mut work, at, &def);
            }
        }
        simplify(&mut work);
    }
    work.finish()
}

/// Node `at` (of `def`) cut: removed, its name kept.
fn cut(work: &mut Work, at: NodeIndex, def: &kinogaki_mtlx::NodeDef) {
    work.take(at);
    work.cut.insert(Arc::from(def.node.as_str()));
}

/// Shading model `model`'s surface definition: its default version.
fn model(model: &str) -> Option<Arc<NodeDef>> {
    let surface = Type::of("surfaceshader");
    let signatures = || library().signatures(model).filter(|d| d.ty() == surface);
    let def = (signatures().find(|d| d.default_version || d.version.is_none()))
        .or_else(|| signatures().next())?;
    mtlx::def(&def.name)
}

/// Bypasses, folds and leaves out what the readers would compute again,
/// until nothing more goes.
fn simplify(work: &mut Work) {
    loop {
        let mut changed = false;
        for at in work.live() {
            changed |= bypass(work, at) || fold(work, at) || implied(work, at);
        }
        if !changed {
            return;
        }
    }
}

/// A `dot` reading a link: its readers read what it reads.
fn bypass(work: &mut Work, at: NodeIndex) -> bool {
    let Some(node) = work.node(at) else {
        return false;
    };
    if node.def.mtlx().is_none_or(|d| d.node != "dot") || work.output == Some(at) {
        return false;
    }
    let Some(link) = work.driver(at, "in").cloned() else {
        return false;
    };
    for (_, reader, input) in work.out_of(at) {
        work.link(link.from, &link.output, reader, &input);
    }
    work.take(at);
    true
}

/// A node of values alone, every reader taking a value where it reads it: the
/// value it yields, in its readers' inputs.
fn fold(work: &mut Work, at: NodeIndex) -> bool {
    let Some(node) = work.node(at) else {
        return false;
    };
    let Some(def) = node.def.mtlx().cloned() else {
        return false;
    };
    if work.driven(at) || work.output == Some(at) || def.outputs.len() != 1 {
        return false;
    }
    let Some(value) = def.evaluate(&mtlx_values(&node.values)) else {
        return false;
    };
    let readers = work.out_of(at);
    let kinds: Option<Vec<_>> = (readers.iter())
        .map(|(_, reader, input)| {
            let p = work.node(*reader)?.def.find_input(input)?.value()?;
            mtlx::value(&p.kind, &value).filter(|v| p.kind.accepts(v))
        })
        .collect();
    let Some(values) = kinds else {
        return false;
    };
    work.take(at);
    for ((_, reader, input), v) in readers.into_iter().zip(values) {
        work.set(reader, &input, v);
    }
    true
}

/// A node reading a geometric property (a `texcoord` of set 0, a world
/// `normal`, a flat `normalmap`) whose every reader reads that same one while
/// unwired: gone.
fn implied(work: &mut Work, at: NodeIndex) -> bool {
    let Some(node) = work.node(at) else {
        return false;
    };
    let Some(def) = node.def.mtlx().cloned() else {
        return false;
    };
    if work.driven(at) || work.output == Some(at) {
        return false;
    }
    let values = node.values.clone();
    let held = mtlx_values(&values);
    let text = |name: &str| match held(name) {
        Some(Value::Text(t)) => Some(t),
        _ => None,
    };
    let index = || match held("index") {
        Some(Value::Integer(n)) => n,
        _ => 0,
    };
    let reads = match def.node.as_str() {
        "normalmap" => {
            let flat = held("in").is_some_and(|v| v.numbers() == Some(&[0.5, 0.5, 1.0]));
            let unscaled = held("scale")
                .is_none_or(|v| v.numbers().is_some_and(|n| n.iter().all(|s| *s == 1.0)));
            (flat && unscaled).then(|| "Nworld".to_owned())
        }
        node => (library().geomprops()).find_map(|(name, prop)| {
            let space = prop
                .space
                .as_deref()
                .is_none_or(|s| text("space").as_deref() == Some(s));
            let indexed = prop.index.is_none_or(|i| i == index());
            (prop.node == node && prop.ty == def.ty() && space && indexed).then(|| name.to_owned())
        }),
    };
    let Some(reads) = reads else {
        return false;
    };
    let readers = work.out_of(at);
    let all = (readers.iter()).all(|(_, reader, input)| {
        let port = work
            .node(*reader)
            .and_then(|n| n.def.mtlx()?.input(input).cloned());
        port.and_then(|p| p.geomprop).as_deref() == Some(reads.as_str())
    });
    if !all || readers.is_empty() {
        return false;
    }
    work.take(at);
    true
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
