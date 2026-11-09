//! Admission: whether a package may register ([`Registry::check`]).
use super::{Registry, owner, standard_type};
use crate::Error;
use crate::cook::SPARE;
use crate::id::{ContextId, DataTypeId, NodeTypeId, PackageId};
use crate::network::Evaluation;
use crate::package::Package;
use crate::params::{Schema, ValueKind};
use kinogaki_render_api::aov;
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
    /// declares a parameter named as spares are ([`SPARE`]), holding an
    /// object ([`ValueKind::Object`]) or an image ([`ValueKind::Image`]),
    /// each compiler
    /// and runner it brings serves a compiled context (its own or a
    /// registered package's) that has no other, every data type
    /// it names is registered (by it or an earlier package), and each
    /// engine's own prim types are new and under its vendor (the standard
    /// types are any engine's) and the standard outputs it declares are the
    /// standard ones, and every schema it declares (its node types'
    /// parameters, its engines' settings, prim types and material nodes) is
    /// whole ([`Schema::check`]), and every home it names fits where it is
    /// listed (`check_homes`: a topic of each place it reaches, or its own
    /// family, which core has none of). Changes nothing.
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
        let mut verbs = BTreeSet::new();
        for v in &package.verbs {
            let offered = crate::kernel::offered_by(v.id()).filter(|&by| by != id);
            let twice = (!verbs.insert(v.id())).then_some(id);
            declare("verb", v.id().as_str(), offered.or(twice))?;
        }
        if let Some(by) = crate::script::offered_by().filter(|&by| by != id)
            && package.script_engine.is_some()
        {
            return Err(Error::refused(format!(
                "{id} brings a script engine, which {by} brings"
            )));
        }
        for m in &package.meshers {
            let first = self.meshers.iter().find(|e| e.item.id() == m.id());
            declare("mesher", m.id().as_str(), owner(first))?;
        }
        for s in &package.subdividers {
            let first = self.subdividers.iter().find(|e| e.item.id() == s.id());
            declare("subdivider", s.id().as_str(), owner(first))?;
        }
        for k in &package.object_kinds {
            declare(
                "object kind",
                k.id().as_str(),
                owner(self.object_kinds.get(&k.id())),
            )?;
        }
        for e in &package.engines {
            let first = self.engines.get(&e.id()).copied();
            declare("render engine", declared_id(id, e.id().as_str())?, first)?;
            for ty in (e.capabilities().types.iter()).filter(|t| !standard_type(t.id)) {
                let first = self.prim_types.get(&ty.id).copied();
                declare("prim type", ty.id.as_str(), first)?;
            }
            for spec in &e.capabilities().aovs {
                if let Some(standard) = aov::standard::get(&spec.aov.id)
                    && standard != spec.aov
                {
                    return Err(Error::refused(format_args!(
                        "{id}: render engine {} declares output {} unlike the standard one",
                        e.id(),
                        spec.aov.id
                    )));
                }
            }
            let own = kinogaki_render_api::settings::OUTPUT_SETTINGS;
            if let Some(d) = (e.settings().parameters()).find(|d| own.contains(&&*d.name)) {
                return Err(Error::refused(format_args!(
                    "{id}: render engine {} has a setting named {}, as a render output's own",
                    e.id(),
                    d.name
                )));
            }
            let library = kinogaki_render_api::materialx::library();
            let unknown = |n: &&std::sync::Arc<str>| {
                library.def(n).is_none() && library.signatures(n).next().is_none()
            };
            if let Some(name) = e.capabilities().standard_nodes.iter().find(unknown) {
                return Err(Error::refused(format_args!(
                    "{id}: render engine {} implements MaterialX node {name}, which MaterialX's library doesn't define",
                    e.id()
                )));
            }
            schema(id, &format!("render engine {}", e.id()), e.settings())?;
            for ty in &e.capabilities().types {
                schema(id, &format!("prim type {}", ty.id), ty.schema())?;
            }
            for def in e.node_defs() {
                schema(id, &format!("material node {}", def.id), &def.schema())?;
            }
        }
        // Object creations, under the same rule, after the engines (an
        // engine's Render Output is one): its kinds' (made with every engine
        // registered and its own) and its creations of others' kinds.
        for c in self.object_creations(package)? {
            let first = self.object_creation_ids.get(&c.id).copied();
            declare("object creation", c.id.as_str(), first)?;
        }
        for c in &package.creations {
            let first = self.creations.iter().find(|e| e.item.id == c.id);
            declare("node creation", c.id.as_str(), owner(first))?;
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
            schema(
                id,
                &format!("node type {}", n.id()),
                &description.parameters,
            )?;
            let object = |p: &&crate::params::Parameter| matches!(p.kind, ValueKind::Object(_));
            if let Some(p) = description.parameters.parameters().find(object) {
                return Err(Error::refused(format!(
                    "{id}'s node type {} declares parameter {}, an object row: a node names what it reads by a Reference path",
                    n.id(),
                    p.name
                )));
            }
            let image = |p: &&crate::params::Parameter| p.kind == ValueKind::Image;
            if let Some(p) = description.parameters.parameters().find(image) {
                return Err(Error::refused(format!(
                    "{id}'s node type {} declares parameter {}, an image row: a node reads an image by a wire from an Image node",
                    n.id(),
                    p.name
                )));
            }
        }
        self.check_names(package)?;
        self.check_compilers(package)?;
        self.check_insides(package)?;
        self.check_references(package)?;
        self.check_homes(package)
    }

    /// Every context `package` names is declared, by it or before it
    /// (where its node types are offered, what its object kinds hold),
    /// and each of its contexts adopts a node type at most once.
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
        for n in &package.node_types {
            let d = n.description();
            for &c in &d.contexts {
                context(n.id().as_str(), c)?;
            }
        }
        for k in &package.object_kinds {
            if let Some(c) = k.context() {
                context(k.id().as_str(), c)?;
            }
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
        for v in &package.verbs {
            for port in v.inputs().iter().chain(v.outputs()) {
                for data in port.types() {
                    known(v.id().as_str(), data)?;
                }
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

/// Whether `schema`, of `what` `id` declares, is whole ([`Schema::check`]).
fn schema(id: PackageId, what: &str, schema: &Schema) -> Result<(), Error> {
    schema
        .check()
        .map_err(|why| Error::refused(format_args!("{id}: {what}: {}", why.message())))
}
