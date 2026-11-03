//! [`Registry`]: what every loaded package registered, by id.
//!
//! The environment registers packages in dependency order (the librarian
//! resolves it: kernels before the packages that build on them), so a
//! package may name data types that it or an earlier package registered,
//! never a later one's. A package is checked whole before anything of it
//! is kept: a refused package leaves the registry as it was. A host
//! admits every package through these checks ([`Registry::check`],
//! [`Registry::register`]) and turns a refusal into a plugin it doesn't
//! load, never a crash.
mod check;
mod error;

pub use error::RegistryError;

use crate::cook::NodeType;
use crate::data_type::DataType;
use crate::format::FileFormat;
use crate::held::HeldEdit;
use crate::id::{
    ContextId, DataTypeId, FormatId, NodeTypeId, ObjectKindId, PackageId, SlotId, StoredId,
};
use crate::mesher::Mesher;
use crate::network::{NetworkCompiler, NetworkContext, NodeCreation, NodeTypes};
use crate::object::ObjectKindSpec;
use crate::package::Package;
use crate::session::SessionSlot;
use kinogaki_render_api::EngineId;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// One registered contribution with the package that registered it.
struct Entry<T: ?Sized> {
    package: PackageId,
    item: Arc<T>,
}

impl<T: ?Sized> Entry<T> {
    fn new(package: PackageId, item: Arc<T>) -> Self {
        Self { package, item }
    }
}

/// The package that registered `entry`, when there is one.
fn owner<T: ?Sized>(entry: Option<&Entry<T>>) -> Option<PackageId> {
    entry.map(|e| e.package)
}

/// Every package's registered data types, held edits, node types, network
/// contexts, B-rep meshers, object kinds, formats, session slots
/// and stored values.
#[derive(Default)]
pub struct Registry {
    packages: Vec<(PackageId, String)>,
    data_types: BTreeMap<DataTypeId, Entry<dyn DataType>>,
    /// Held edits by the data type of their deltas.
    held_edits: BTreeMap<DataTypeId, Entry<dyn HeldEdit>>,
    node_types: BTreeMap<NodeTypeId, Entry<dyn NodeType>>,
    contexts: BTreeMap<ContextId, Entry<NetworkContext>>,
    /// Each compiled context's compiler.
    compilers: BTreeMap<ContextId, Entry<dyn NetworkCompiler>>,
    /// Which contexts hold networks of which: an edge from a context a
    /// holding node type is offered in to the context it holds.
    holds: Vec<(ContextId, ContextId)>,
    /// The package bringing each compiled context's runner.
    #[cfg(feature = "wgpu")]
    runners: BTreeMap<ContextId, PackageId>,
    /// Further ways to make nodes, in registration order.
    creations: Vec<Entry<NodeCreation>>,
    /// B-rep meshers, in registration order.
    meshers: Vec<Entry<dyn Mesher>>,
    object_kinds: BTreeMap<ObjectKindId, Entry<dyn ObjectKindSpec>>,
    formats: BTreeMap<FormatId, Entry<dyn FileFormat>>,
    slots: BTreeMap<SlotId, Entry<SessionSlot>>,
    /// The package declaring each stored value.
    stored: BTreeMap<StoredId, PackageId>,
    /// The contexts packages' UIs give parts to, and the package giving
    /// them.
    ui_contexts: BTreeMap<ContextId, PackageId>,
    /// The package describing each render engine, by engine id.
    engines: BTreeMap<EngineId, PackageId>,
    /// Each package's icons' names.
    icons: BTreeMap<PackageId, BTreeSet<String>>,
}

impl Registry {
    /// An empty registry: the environment with no packages.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `package` once [`check`](Self::check) passes. On an
    /// error nothing of the package is kept.
    pub fn register(&mut self, package: Package) -> Result<(), RegistryError> {
        self.check(&package)?;
        let id = package.id;
        self.holds.extend(
            check::holds(&package)
                .into_iter()
                .map(|(from, to, _)| (from, to)),
        );
        for &context in package.ui.iter().flat_map(|ui| ui.contexts()) {
            self.ui_contexts.insert(context, id);
        }
        for ty in package.data_types {
            self.data_types.insert(ty.id(), Entry::new(id, ty));
        }
        for held in package.held_edits {
            self.held_edits.insert(held.delta(), Entry::new(id, held));
        }
        for node in package.node_types {
            self.node_types.insert(node.id(), Entry::new(id, node));
        }
        for context in package.contexts {
            self.contexts
                .insert(context.id, Entry::new(id, Arc::new(context)));
        }
        for compiler in package.compilers {
            self.compilers
                .insert(compiler.context(), Entry::new(id, compiler));
        }
        #[cfg(feature = "wgpu")]
        for (context, _) in &package.runners {
            self.runners.insert(*context, id);
        }
        for creation in package.creations {
            self.creations.push(Entry::new(id, Arc::new(creation)));
        }
        for mesher in package.meshers {
            self.meshers.push(Entry::new(id, mesher));
        }
        for kind in package.object_kinds {
            self.object_kinds.insert(kind.id(), Entry::new(id, kind));
        }
        for format in package.formats {
            self.formats.insert(format.id(), Entry::new(id, format));
        }
        for slot in package.slots {
            self.slots.insert(slot.id(), Entry::new(id, Arc::new(slot)));
        }
        for stored in package.stored {
            self.stored.insert(stored.id(), id);
        }
        for engine in package.engines {
            self.engines.insert(engine.id.clone(), id);
        }
        let icons = package.icons.into_iter().map(|i| i.name).collect();
        self.icons.insert(id, icons);
        self.packages.push((id, package.label));
        Ok(())
    }

    /// Compiled context `context`'s compiler.
    pub fn compiler(&self, context: ContextId) -> Option<&dyn NetworkCompiler> {
        self.compilers.get(&context).map(|e| &*e.item)
    }

    /// The registered packages' ids and labels, in registration order.
    pub fn packages(&self) -> impl Iterator<Item = (PackageId, &str)> {
        self.packages
            .iter()
            .map(|(id, label)| (*id, label.as_str()))
    }

    /// Data type `id`; `None` when no package registered it (a host skips
    /// its values when it reads a project, and says so).
    pub fn data_type(&self, id: DataTypeId) -> Option<&dyn DataType> {
        self.data_types.get(&id).map(|e| &*e.item)
    }

    /// Every data type, by id.
    pub fn data_types(&self) -> impl Iterator<Item = &dyn DataType> {
        self.data_types.values().map(|e| &*e.item)
    }

    /// How deltas of type `delta` apply, when a package says: a held
    /// node's type names its deltas' type ([`NodeType::held`]).
    pub fn held_edit(&self, delta: DataTypeId) -> Option<&dyn HeldEdit> {
        self.held_edits.get(&delta).map(|e| &*e.item)
    }

    /// What node type `id` computes; `None` when no package registered it
    /// (a host skips its nodes when it reads a project, and says so).
    pub fn node_type(&self, id: NodeTypeId) -> Option<&dyn NodeType> {
        self.node_types.get(&id).map(|e| &*e.item)
    }

    /// The further ways packages make nodes
    /// ([`Package::with_creation`]), in registration order.
    pub fn creations(&self) -> impl Iterator<Item = &NodeCreation> {
        self.creations.iter().map(|e| &*e.item)
    }

    /// The B-rep meshers, the preferred first: first-party (`kinogaki.*`)
    /// packages' before others', each in registration order.
    pub fn meshers(&self) -> impl Iterator<Item = &dyn Mesher> {
        let first_party = |e: &&Entry<dyn Mesher>| e.package.vendor() == "kinogaki";
        let ours = self.meshers.iter().filter(first_party);
        let theirs = self.meshers.iter().filter(move |e| !first_party(e));
        ours.chain(theirs).map(|e| &*e.item)
    }

    /// Object kind `id`.
    pub fn object_kind(&self, id: ObjectKindId) -> Option<&dyn ObjectKindSpec> {
        self.object_kinds.get(&id).map(|e| &*e.item)
    }

    /// Every object kind, by id.
    pub fn object_kinds(&self) -> impl Iterator<Item = &dyn ObjectKindSpec> {
        self.object_kinds.values().map(|e| &*e.item)
    }

    /// Format `id`.
    pub fn format(&self, id: FormatId) -> Option<&dyn FileFormat> {
        self.formats.get(&id).map(|e| &*e.item)
    }

    /// Every format, by id.
    pub fn formats(&self) -> impl Iterator<Item = &dyn FileFormat> {
        self.formats.values().map(|e| &*e.item)
    }

    /// Session slot `id`.
    pub fn slot(&self, id: SlotId) -> Option<SessionSlot> {
        self.slots.get(&id).map(|e| *e.item)
    }
}

/// A compile sees the registered node types.
impl NodeTypes for Registry {
    fn node_type(&self, id: NodeTypeId) -> Option<&dyn NodeType> {
        Registry::node_type(self, id)
    }
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("packages", &self.packages)
            .field("data_types", &self.data_types.keys().collect::<Vec<_>>())
            .field("held_edits", &self.held_edits.keys().collect::<Vec<_>>())
            .field("node_types", &self.node_types.keys().collect::<Vec<_>>())
            .field(
                "object_kinds",
                &self.object_kinds.keys().collect::<Vec<_>>(),
            )
            .field("formats", &self.formats.keys().collect::<Vec<_>>())
            .field("slots", &self.slots.keys().collect::<Vec<_>>())
            .field("stored", &self.stored.keys().collect::<Vec<_>>())
            .finish()
    }
}
