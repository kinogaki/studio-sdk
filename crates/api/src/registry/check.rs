//! Admission: whether a package may register ([`Registry::check`]).
use super::{Registry, RegistryError, owner};
use crate::cook::SPARE;
use crate::id::{ContextId, DataTypeId, NodeTypeId, PackageId};
use crate::network::Evaluation;
use crate::object::{CreateCx, ObjectKindSpec};
use crate::package::Package;
use kinogaki_render_api::Engines;
use std::collections::{BTreeMap, BTreeSet};

/// Creation name `text` of `package` as an id's text, when well formed.
fn creation_id(package: PackageId, text: &str) -> Result<&'static str, RegistryError> {
    if crate::id::well_formed(text) {
        Ok(crate::id::intern(text))
    } else {
        Err(RegistryError::MalformedId {
            package,
            id: text.to_owned(),
        })
    }
}

impl Registry {
    /// Whether `package` may register: its id and every id it declares
    /// (its UI's contexts too, and its creations' names: its kinds' with
    /// no engine installed, and its node creations', each well formed)
    /// are new and under its vendor, no node type
    /// declares a parameter named as spares are ([`SPARE`]), each compiler
    /// and runner it brings serves a compiled context (its own or a
    /// registered package's) that has no other, and every data type
    /// it names is registered (by it or an earlier package). Changes
    /// nothing.
    pub fn check(&self, package: &Package) -> Result<(), RegistryError> {
        let id = package.id;
        if self.packages.iter().any(|(p, _)| *p == id) {
            return Err(RegistryError::DuplicatePackage(id));
        }
        // Ids declared so far, per kind, by this package.
        let mut seen: BTreeSet<(&'static str, &'static str)> = BTreeSet::new();
        let mut declare = |what: &'static str,
                           text: &'static str,
                           first: Option<PackageId>|
         -> Result<(), RegistryError> {
            if text.split('.').next() != Some(id.vendor()) {
                return Err(RegistryError::ForeignId {
                    package: id,
                    id: text,
                });
            }
            match first {
                Some(first) => Err(RegistryError::DuplicateId {
                    package: id,
                    what,
                    id: text,
                    first,
                }),
                None if !seen.insert((what, text)) => Err(RegistryError::DuplicateId {
                    package: id,
                    what,
                    id: text,
                    first: id,
                }),
                None => Ok(()),
            }
        };
        for ty in &package.data_types {
            declare(
                "data type",
                ty.id().as_str(),
                owner(self.data_types.get(&ty.id())),
            )?;
        }
        for n in &package.node_types {
            declare(
                "node type",
                n.id().as_str(),
                owner(self.node_types.get(&n.id())),
            )?;
        }
        for c in &package.contexts {
            declare(
                "network context",
                c.id.as_str(),
                owner(self.contexts.get(&c.id)),
            )?;
        }
        for c in &package.conversions {
            declare(
                "conversion",
                c.id().as_str(),
                owner(self.conversions.get(&c.id())),
            )?;
        }
        for m in &package.meshers {
            let first = self.meshers.iter().find(|e| e.item.id() == m.id());
            declare("mesher", m.id().as_str(), owner(first))?;
        }
        for k in &package.object_kinds {
            declare(
                "object kind",
                k.id().as_str(),
                owner(self.object_kinds.get(&k.id())),
            )?;
        }
        // Creations are named by text, under the same rule. A kind's are
        // those it makes with no engine installed; the rest are named by
        // the engines' own types.
        let engines = Engines::new(Vec::new());
        let cx = CreateCx::new(&engines, None);
        let made = |k: &dyn ObjectKindSpec| k.creations(&cx).into_iter().map(|c| c.id);
        for text in package.object_kinds.iter().flat_map(|k| made(&**k)) {
            let first = (self.object_kinds.values())
                .find(|e| made(&*e.item).any(|t| t == text))
                .map(|e| e.package);
            declare("object creation", creation_id(id, &text)?, first)?;
        }
        for c in &package.creations {
            let first = self.creations.iter().find(|e| e.item.id == c.id);
            declare("node creation", creation_id(id, &c.id)?, owner(first))?;
        }
        for f in &package.formats {
            declare("format", f.id().as_str(), owner(self.formats.get(&f.id())))?;
        }
        for s in &package.slots {
            declare(
                "session slot",
                s.id().as_str(),
                owner(self.slots.get(&s.id())),
            )?;
        }
        for s in &package.stored {
            declare(
                "stored value",
                s.id().as_str(),
                self.stored.get(&s.id()).copied(),
            )?;
        }
        for &(context, mode) in package.ui.iter().flat_map(|ui| ui.contexts()) {
            let what = if mode { "mode's UI" } else { "context's UI" };
            declare(
                what,
                context.as_str(),
                self.ui_contexts.get(&(context, mode)).copied(),
            )?;
        }
        for n in &package.node_types {
            let description = n.description();
            if let Some(p) = description
                .parameters
                .parameters()
                .find(|p| p.name.starts_with(SPARE))
            {
                return Err(RegistryError::SpareName {
                    package: id,
                    node: n.id(),
                    parameter: p.name.clone(),
                });
            }
        }
        self.check_engines(package)?;
        self.check_compilers(package)?;
        self.check_insides(package)?;
        self.check_references(package)
    }

    /// Each render engine `package` describes is new (engine ids are
    /// files' names, `pixel`, under no vendor), and each maker it brings
    /// makes one it describes.
    fn check_engines(&self, package: &Package) -> Result<(), RegistryError> {
        let id = package.id;
        for (at, engine) in package.engines.iter().enumerate() {
            let twice = package.engines[..at].iter().any(|e| e.id == engine.id);
            let first = self.engines.get(&engine.id).copied();
            if let Some(first) = first.or(twice.then_some(id)) {
                return Err(RegistryError::DuplicateEngine {
                    package: id,
                    engine: engine.id.clone(),
                    first,
                });
            }
        }
        #[cfg(feature = "wgpu")]
        for maker in &package.makers {
            let engine = maker.description().id;
            if !package.engines.iter().any(|e| e.id == engine) {
                return Err(RegistryError::StrayMaker {
                    package: id,
                    engine,
                });
            }
        }
        Ok(())
    }

    /// Each compiler and runner `package` brings serves a compiled context
    /// it or a registered package declares (core's Comp and Shading), and
    /// no context gets a second of either. A compiled context may have
    /// none: its plans then aren't shown.
    fn check_compilers(&self, package: &Package) -> Result<(), RegistryError> {
        let id = package.id;
        let compiled = |context: ContextId| {
            package
                .contexts
                .iter()
                .chain(self.contexts.values().map(|e| &*e.item))
                .any(|c| c.id == context && c.evaluation == Evaluation::Compile)
        };
        let mut given = BTreeSet::new();
        for c in &package.compilers {
            let context = c.context();
            if !compiled(context) {
                return Err(RegistryError::StrayCompiler {
                    package: id,
                    context,
                });
            }
            let first =
                owner(self.compilers.get(&context)).or((!given.insert(context)).then_some(id));
            if let Some(first) = first {
                return Err(RegistryError::DuplicateCompiler {
                    package: id,
                    context,
                    first,
                });
            }
        }
        #[cfg(feature = "wgpu")]
        {
            let mut run = BTreeSet::new();
            for (context, _) in &package.runners {
                let context = *context;
                if !compiled(context) {
                    return Err(RegistryError::StrayRunner {
                        package: id,
                        context,
                    });
                }
                let first = self
                    .runners
                    .get(&context)
                    .copied()
                    .or((!run.insert(context)).then_some(id));
                if let Some(first) = first {
                    return Err(RegistryError::DuplicateRunner {
                        package: id,
                        context,
                        first,
                    });
                }
            }
        }
        Ok(())
    }

    /// Each network `package`'s node types hold inside them is of a
    /// declared context, and no context comes to hold its own networks
    /// through others' (Houdini's nesting stays possible; recursion
    /// doesn't).
    fn check_insides(&self, package: &Package) -> Result<(), RegistryError> {
        let id = package.id;
        let new = holds(package);
        for &(_, inside, node) in &new {
            let declared = package.contexts.iter().any(|c| c.id == inside)
                || self.contexts.contains_key(&inside);
            if !declared {
                return Err(RegistryError::UnknownContext {
                    package: id,
                    node,
                    context: inside,
                });
            }
        }
        let mut edges = self.holds.clone();
        edges.extend(new.iter().map(|&(from, to, _)| (from, to)));
        for &(from, to, node) in &new {
            if let Some(back) = route(&edges, to, from) {
                let mut path = vec![from];
                path.extend(back);
                return Err(RegistryError::InsideCycle {
                    package: id,
                    node,
                    path,
                });
            }
        }
        Ok(())
    }

    /// Every data type `package` names is registered, by it or before it,
    /// and no (from, to) pair gets a second conversion.
    fn check_references(&self, package: &Package) -> Result<(), RegistryError> {
        let id = package.id;
        let own: BTreeSet<DataTypeId> = package.data_types.iter().map(|t| t.id()).collect();
        let known = |by: &'static str, data: DataTypeId| {
            if own.contains(&data) || self.data_types.contains_key(&data) {
                Ok(())
            } else {
                Err(RegistryError::UnknownDataType {
                    package: id,
                    by,
                    data,
                })
            }
        };
        let mut deltas = BTreeSet::new();
        for h in &package.held_edits {
            known("held edit", h.base())?;
            known("held edit", h.delta())?;
            if let Some(first) =
                owner(self.held_edits.get(&h.delta())).or((!deltas.insert(h.delta())).then_some(id))
            {
                return Err(RegistryError::DuplicateHeldEdit {
                    package: id,
                    delta: h.delta(),
                    first,
                });
            }
        }
        for n in &package.node_types {
            if let Some(delta) = n.held() {
                known(n.id().as_str(), delta)?;
            }
        }
        let mut routes = BTreeMap::new();
        for c in &package.conversions {
            known(c.id().as_str(), c.from())?;
            known(c.id().as_str(), c.to())?;
            let route = (c.from(), c.to());
            if let Some(&first) = self.routes.get(&route).or(routes.get(&route)) {
                return Err(RegistryError::DuplicateConversion {
                    package: id,
                    from: route.0,
                    to: route.1,
                    first,
                });
            }
            routes.insert(route, c.id());
        }
        for k in &package.object_kinds {
            if let Some(result) = k.result() {
                known(k.id().as_str(), result)?;
            }
        }
        for f in &package.formats {
            for &data in f.produces() {
                known(f.id().as_str(), data)?;
            }
            for &data in f.writer().map_or(&[][..], |w| w.accepts()) {
                known(f.id().as_str(), data)?;
            }
        }
        Ok(())
    }
}

/// The contexts `package`'s node types hold networks of: (offered in,
/// held, node type), one for each context a holding type is offered in.
pub(super) fn holds(package: &Package) -> Vec<(ContextId, ContextId, NodeTypeId)> {
    let mut out = Vec::new();
    for t in &package.node_types {
        let d = t.description();
        if let Some(inside) = d.inside {
            out.extend(d.contexts.iter().map(|&from| (from, inside, t.id())));
        }
    }
    out
}

/// A way along `edges` from `from` to `to` (both ends included), if any.
fn route(
    edges: &[(ContextId, ContextId)],
    from: ContextId,
    to: ContextId,
) -> Option<Vec<ContextId>> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![vec![from]];
    while let Some(path) = stack.pop() {
        let at = *path.last().expect("a path has an end");
        if at == to {
            return Some(path);
        }
        if !seen.insert(at) {
            continue;
        }
        for &(a, b) in edges {
            if a == at {
                let mut next = path.clone();
                next.push(b);
                stack.push(next);
            }
        }
    }
    None
}
