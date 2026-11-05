//! Admission: whether a package may register ([`Registry::check`]).
use super::{Registry, owner};
use crate::Error;
use crate::cook::SPARE;
use crate::id::{ContextId, DataTypeId, NodeTypeId, PackageId};
use crate::network::Evaluation;
use crate::object::{CreateCx, ObjectKindSpec};
use crate::package::Package;
use crate::params::ValueKind;
use kinogaki_render_api::Engines;
use std::collections::BTreeSet;

/// Id `text` `package` declares as an id's text, when well formed (a
/// creation's name, an engine's id: text, not yet an id type).
fn declared_id(package: PackageId, text: &str) -> Result<&'static str, Error> {
    crate::id::parse_text(text).ok_or_else(|| {
        Error::refused(format!(
            "{package} declares {id:?}, which isn't an id",
            package = package,
            id = text.to_owned()
        ))
    })
}

impl Registry {
    /// Whether `package` may register: its id and every id it declares
    /// (its UI's contexts too, and its creations' names: its kinds' with
    /// no engine installed, and its node creations', each well formed)
    /// are new and under its vendor, no node type
    /// declares a parameter named as spares are ([`SPARE`]) or holding an
    /// object ([`ValueKind::Object`]), each compiler
    /// and runner it brings serves a compiled context (its own or a
    /// registered package's) that has no other, and every data type
    /// it names is registered (by it or an earlier package). Changes
    /// nothing.
    pub fn check(&self, package: &Package) -> Result<(), Error> {
        let id = package.id;
        if self.packages.iter().any(|(p, _)| *p == id) {
            return Err(Error::refused(format!(
                "package {id} is already registered"
            )));
        }
        // Ids declared so far, per kind, by this package.
        let mut seen: BTreeSet<(&'static str, &'static str)> = BTreeSet::new();
        let mut declare = |what: &'static str,
                           text: &'static str,
                           first: Option<PackageId>|
         -> Result<(), Error> {
            if text.split('.').next() != Some(id.vendor()) {
                return Err(Error::refused(format!(
                    "{package} declares {id}, under another vendor's name",
                    package = id,
                    id = text
                )));
            }
            match first {
                Some(first) => Err(Error::refused(format!(
                    "{package} declares {what} {id}, which {first} declared",
                    package = id,
                    what = what,
                    id = text,
                    first = first
                ))),
                None if !seen.insert((what, text)) => Err(Error::refused(format!(
                    "{package} declares {what} {id}, which {first} declared",
                    package = id,
                    what = what,
                    id = text,
                    first = id
                ))),
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
        for v in &package.verbs {
            declare("verb", v.id().as_str(), self.verbs.get(&v.id()).copied())?;
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
        // Creations, under the same rule. A kind's are those it makes with
        // no engine installed; the rest are named by the engines' own
        // types.
        let engines = Engines::new(Vec::new());
        let cx = CreateCx::new(&engines, None);
        let made = |k: &dyn ObjectKindSpec| k.creations(&cx).into_iter().map(|c| c.id);
        for text in package.object_kinds.iter().flat_map(|k| made(&**k)) {
            let first = (self.object_kinds.values())
                .find(|e| made(&*e.item).any(|t| t == text))
                .map(|e| e.package);
            declare("object creation", text.as_str(), first)?;
        }
        for c in &package.creations {
            let first = self.creations.iter().find(|e| e.item.id == c.id);
            declare("node creation", c.id.as_str(), owner(first))?;
        }
        for e in &package.engines {
            let first = self.engines.get(&e.id()).copied();
            declare("render engine", declared_id(id, e.id().as_str())?, first)?;
            let own = crate::RenderSetup::settings();
            if let Some(d) = (e.settings().iter()).find(|d| own.iter().any(|o| o.name == d.name)) {
                return Err(Error::refused(format_args!(
                    "{id}: render engine {} has a setting named {}, as a render setup's own",
                    e.id(),
                    d.name
                )));
            }
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
        for &context in package.ui.iter().flat_map(|ui| ui.contexts()) {
            declare(
                "context's UI",
                context.as_str(),
                self.ui_contexts.get(&context).copied(),
            )?;
        }
        for n in &package.node_types {
            let description = n.description();
            if let Some(p) = description
                .parameters
                .parameters()
                .find(|p| p.name.starts_with(SPARE))
            {
                return Err(Error::refused(format!(
                    "{package}'s node type {node} declares parameter {parameter}, a name spares take",
                    package = id,
                    node = n.id(),
                    parameter = p.name.clone()
                )));
            }
            let object = |p: &&crate::params::Parameter| matches!(p.kind, ValueKind::Object(_));
            if let Some(p) = description.parameters.parameters().find(object) {
                return Err(Error::refused(format!(
                    "{id}'s node type {} declares parameter {}, an object row: a node names what it reads by a Reference path",
                    n.id(),
                    p.name
                )));
            }
        }
        #[cfg(feature = "wgpu")]
        self.check_makers(package)?;
        self.check_names(package)?;
        self.check_compilers(package)?;
        self.check_insides(package)?;
        self.check_references(package)
    }

    /// Each render engine maker `package` brings makes an engine it
    /// describes.
    #[cfg(feature = "wgpu")]
    fn check_makers(&self, package: &Package) -> Result<(), Error> {
        for maker in &package.makers {
            let engine = maker.description().id();
            if !package.engines.iter().any(|e| e.id() == engine) {
                return Err(Error::refused(format!(
                    "{package} makes render engine {engine}, which it doesn't describe",
                    package = package.id,
                    engine = engine
                )));
            }
        }
        Ok(())
    }

    /// Every context `package` names is declared, by it or before it
    /// (where its node types are offered, what its object kinds hold),
    /// each of its contexts adopts a node type at most once, its icons are well formed ([`Icon::check`](crate::icon::Icon::check)) and named once,
    /// and every package icon it names (`"acme.trees/leaf"`) is one it or
    /// a registered package carries.
    fn check_names(&self, package: &Package) -> Result<(), Error> {
        let id = package.id;
        let context = |by: &'static str, context: ContextId| {
            if package.contexts.iter().any(|c| c.id == context)
                || self.contexts.contains_key(&context)
            {
                Ok(())
            } else {
                Err(Error::refused(format!(
                    "{package}'s {by} names context {context}, which no package declares",
                    package = id,
                    by = by,
                    context = context
                )))
            }
        };
        for c in &package.contexts {
            let mut adopted = BTreeSet::new();
            for a in &c.adopts {
                if !adopted.insert(a.node_type) {
                    return Err(Error::refused(format!(
                        "{package}'s {context} adopts {node} twice",
                        package = id,
                        context = c.id,
                        node = a.node_type
                    )));
                }
            }
        }
        let mut names = BTreeSet::new();
        for icon in &package.icons {
            icon.check()
                .map_err(|e| Error::refused(format_args!("{id}'s icon: {e}")))?;
            if !names.insert(icon.name.as_str()) {
                let name = &icon.name;
                return Err(Error::refused(format_args!(
                    "{id}'s icon: a second icon is named {name:?}"
                )));
            }
        }
        let icon = |by: &'static str, name: &str| match name.split_once('/') {
            Some((p, n)) if p == id.as_str() && names.contains(n) => Ok(()),
            Some((p, n)) if (self.icons.iter()).any(|(q, s)| q.as_str() == p && s.contains(n)) => {
                Ok(())
            }
            Some(_) => Err(Error::refused(format!(
                "{package}'s {by} shows icon {icon}, which no package carries",
                package = id,
                by = by,
                icon = name.to_owned()
            ))),
            None => Ok(()),
        };
        for n in &package.node_types {
            let d = n.description();
            for &c in &d.contexts {
                context(n.id().as_str(), c)?;
            }
            icon(n.id().as_str(), &d.icon)?;
        }
        for k in &package.object_kinds {
            if let Some(c) = k.context() {
                context(k.id().as_str(), c)?;
            }
            icon(k.id().as_str(), k.icon())?;
        }
        Ok(())
    }

    /// Each compiler and runner `package` brings serves a compiled context
    /// it or a registered package declares (core's Comp and Shading), and
    /// no context gets a second of either. A compiled context may have
    /// none: its plans then aren't shown.
    fn check_compilers(&self, package: &Package) -> Result<(), Error> {
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
                return Err(Error::refused(format!(
                    "{package} brings a compiler for {context}, which no package declares as a compiled context",
                    package = id,
                    context = context
                )));
            }
            let first =
                owner(self.compilers.get(&context)).or((!given.insert(context)).then_some(id));
            if let Some(first) = first {
                return Err(Error::refused(format!(
                    "{package} brings a compiler for {context}, which {first} already brought",
                    package = id,
                    context = context,
                    first = first
                )));
            }
        }
        #[cfg(feature = "wgpu")]
        {
            let mut run = BTreeSet::new();
            for (context, _) in &package.runners {
                let context = *context;
                if !compiled(context) {
                    return Err(Error::refused(format!(
                        "{package} brings a runner for {context}, which no package declares as a compiled context",
                        package = id,
                        context = context
                    )));
                }
                let first = self
                    .runners
                    .get(&context)
                    .copied()
                    .or((!run.insert(context)).then_some(id));
                if let Some(first) = first {
                    return Err(Error::refused(format!(
                        "{package} brings a runner for {context}, which {first} already brought",
                        package = id,
                        context = context,
                        first = first
                    )));
                }
            }
        }
        Ok(())
    }

    /// Each network `package`'s node types hold inside them is of a
    /// declared context, and no context comes to hold its own networks
    /// through others' (Houdini's nesting stays possible; recursion
    /// doesn't).
    fn check_insides(&self, package: &Package) -> Result<(), Error> {
        let id = package.id;
        let new = holds(package);
        for &(_, inside, node) in &new {
            let declared = package.contexts.iter().any(|c| c.id == inside)
                || self.contexts.contains_key(&inside);
            if !declared {
                return Err(Error::refused(format!(
                    "{package}'s {by} names context {context}, which no package declares",
                    package = id,
                    by = node.as_str(),
                    context = inside
                )));
            }
        }
        let mut edges = self.holds.clone();
        edges.extend(new.iter().map(|&(from, to, _)| (from, to)));
        for &(from, to, node) in &new {
            if let Some(back) = route(&edges, to, from) {
                let mut path = vec![from];
                path.extend(back);
                let path: Vec<&str> = path.iter().map(|c| c.as_str()).collect();
                return Err(Error::refused(format_args!(
                    "{id}'s {node} would make contexts hold each other: {}",
                    path.join(" → ")
                )));
            }
        }
        Ok(())
    }

    /// Every data type `package` names is registered, by it or before it.
    fn check_references(&self, package: &Package) -> Result<(), Error> {
        let id = package.id;
        let own: BTreeSet<DataTypeId> = package.data_types.iter().map(|t| t.id()).collect();
        let known = |by: &'static str, data: DataTypeId| {
            if own.contains(&data) || self.data_types.contains_key(&data) {
                Ok(())
            } else {
                Err(Error::refused(format!(
                    "{package}'s {by} names data type {data}, which isn't registered",
                    package = id,
                    by = by,
                    data = data
                )))
            }
        };
        let mut deltas = BTreeSet::new();
        for h in &package.held_edits {
            known("held edit", h.base())?;
            known("held edit", h.delta())?;
            if let Some(first) =
                owner(self.held_edits.get(&h.delta())).or((!deltas.insert(h.delta())).then_some(id))
            {
                return Err(Error::refused(format!(
                    "{package} declares how {delta} deltas apply, which {first} declares",
                    package = id,
                    delta = h.delta(),
                    first = first
                )));
            }
        }
        for n in &package.node_types {
            if let Some(delta) = n.held() {
                known(n.id().as_str(), delta)?;
            }
            let d = n.description();
            for port in d.inputs.iter().chain(&d.outputs) {
                for data in port.types() {
                    known(n.id().as_str(), data)?;
                }
            }
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
