//! [`Registry`]: what every loaded package registered, by id.
//!
//! The environment registers packages in dependency order (the librarian
//! resolves it: kernels before the packages that build on them), so a
//! package may name data types that it or an earlier package registered,
//! never a later one's. A package is checked whole before anything of it
//! is kept: a refused package leaves the registry as it was.
use crate::conversion::Conversion;
use crate::cook::NodeType;
use crate::data_type::DataType;
use crate::format::FileFormat;
use crate::held::HeldEdit;
use crate::id::{ConversionId, DataTypeId, FormatId, NodeTypeId, ObjectKindId, PackageId, SlotId};
use crate::object::ObjectKindSpec;
use crate::package::Package;
use crate::session::{SessionSlot, SessionSlots};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// Why a package was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// A package with this id is already registered.
    DuplicatePackage(PackageId),
    /// `package` declares `id` (a `what`), which `first` already declared
    /// (or `package` declares twice).
    DuplicateId {
        /// The refused package.
        package: PackageId,
        /// What kind of id ("data type", "format").
        what: &'static str,
        /// The id.
        id: &'static str,
        /// The package that declared it first.
        first: PackageId,
    },
    /// `package` declares `id`, which is under another vendor's name.
    ForeignId {
        /// The refused package.
        package: PackageId,
        /// The id.
        id: &'static str,
    },
    /// `by` (an id `package` declares) names data type `data`, which no
    /// package registered before it.
    UnknownDataType {
        /// The refused package.
        package: PackageId,
        /// What names the type.
        by: &'static str,
        /// The missing type.
        data: DataTypeId,
    },
    /// `package` declares how deltas apply to `base`, which `first`
    /// already declares.
    DuplicateHeldEdit {
        /// The refused package.
        package: PackageId,
        /// The data type the edits apply to.
        base: DataTypeId,
        /// The package that declared it first.
        first: PackageId,
    },
    /// `package` declares a second conversion from `from` to `to`; `first`
    /// is the one already registered.
    DuplicateConversion {
        /// The refused package.
        package: PackageId,
        /// The type converted from.
        from: DataTypeId,
        /// The type converted to.
        to: DataTypeId,
        /// The conversion registered first.
        first: ConversionId,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePackage(id) => write!(f, "package {id} is already registered"),
            Self::DuplicateId {
                package,
                what,
                id,
                first,
            } => write!(f, "{package} declares {what} {id}, which {first} declared"),
            Self::ForeignId { package, id } => {
                write!(f, "{package} declares {id}, under another vendor's name")
            }
            Self::UnknownDataType { package, by, data } => {
                write!(
                    f,
                    "{package}'s {by} names data type {data}, which isn't registered"
                )
            }
            Self::DuplicateHeldEdit {
                package,
                base,
                first,
            } => write!(
                f,
                "{package} declares held edits of {base}, which {first} declares"
            ),
            Self::DuplicateConversion {
                package,
                from,
                to,
                first,
            } => write!(
                f,
                "{package} converts {from} to {to}, which {first} already does"
            ),
        }
    }
}

impl std::error::Error for RegistryError {}

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

/// Every package's registered data types, conversions, object kinds,
/// formats and session slots.
#[derive(Default)]
pub struct Registry {
    packages: Vec<(PackageId, String)>,
    data_types: BTreeMap<DataTypeId, Entry<dyn DataType>>,
    /// Each base data type's held edits.
    held_edits: BTreeMap<DataTypeId, Entry<dyn HeldEdit>>,
    node_types: BTreeMap<NodeTypeId, Entry<dyn NodeType>>,
    conversions: BTreeMap<ConversionId, Entry<dyn Conversion>>,
    /// Each (from, to) pair's conversion.
    routes: BTreeMap<(DataTypeId, DataTypeId), ConversionId>,
    object_kinds: BTreeMap<ObjectKindId, Entry<dyn ObjectKindSpec>>,
    formats: BTreeMap<FormatId, Entry<dyn FileFormat>>,
    slots: BTreeMap<SlotId, Entry<dyn SessionSlot>>,
}

impl Registry {
    /// An empty registry: the environment with no packages.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `package`, after checking that its id and every id it
    /// declares are new, under its vendor, and that every data type it
    /// names is registered (by it or an earlier package). On an error
    /// nothing of the package is kept.
    pub fn register(&mut self, package: Package) -> Result<(), RegistryError> {
        self.check(&package)?;
        let id = package.id;
        for ty in package.data_types {
            self.data_types.insert(ty.id(), Entry::new(id, ty));
        }
        for held in package.held_edits {
            self.held_edits.insert(held.base(), Entry::new(id, held));
        }
        for node in package.node_types {
            self.node_types.insert(node.id(), Entry::new(id, node));
        }
        for conversion in package.conversions {
            self.routes
                .insert((conversion.from(), conversion.to()), conversion.id());
            self.conversions
                .insert(conversion.id(), Entry::new(id, conversion));
        }
        for kind in package.object_kinds {
            self.object_kinds.insert(kind.id(), Entry::new(id, kind));
        }
        for format in package.formats {
            self.formats.insert(format.id(), Entry::new(id, format));
        }
        for slot in package.slots {
            self.slots.insert(slot.id(), Entry::new(id, slot));
        }
        self.packages.push((id, package.label));
        Ok(())
    }

    /// Every check [`register`](Self::register) makes, without changing
    /// anything.
    fn check(&self, package: &Package) -> Result<(), RegistryError> {
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
        for c in &package.conversions {
            declare(
                "conversion",
                c.id().as_str(),
                owner(self.conversions.get(&c.id())),
            )?;
        }
        for k in &package.object_kinds {
            declare(
                "object kind",
                k.id().as_str(),
                owner(self.object_kinds.get(&k.id())),
            )?;
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
        self.check_references(package)
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
        let mut bases = BTreeSet::new();
        for h in &package.held_edits {
            known("held edit", h.base())?;
            known("held edit", h.delta())?;
            if let Some(first) =
                owner(self.held_edits.get(&h.base())).or((!bases.insert(h.base())).then_some(id))
            {
                return Err(RegistryError::DuplicateHeldEdit {
                    package: id,
                    base: h.base(),
                    first,
                });
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

    /// The registered packages' ids and labels, in registration order.
    pub fn packages(&self) -> impl Iterator<Item = (PackageId, &str)> {
        self.packages
            .iter()
            .map(|(id, label)| (*id, label.as_str()))
    }

    /// Data type `id`; `None` when no package registered it (its values
    /// are kept as they were, never shown).
    pub fn data_type(&self, id: DataTypeId) -> Option<&dyn DataType> {
        self.data_types.get(&id).map(|e| &*e.item)
    }

    /// Every data type, by id.
    pub fn data_types(&self) -> impl Iterator<Item = &dyn DataType> {
        self.data_types.values().map(|e| &*e.item)
    }

    /// How deltas apply to values of `base`, when a package says.
    pub fn held_edit(&self, base: DataTypeId) -> Option<&dyn HeldEdit> {
        self.held_edits.get(&base).map(|e| &*e.item)
    }

    /// What node type `id` computes; `None` when no package registered it
    /// (its nodes are kept as placeholders and never cooked).
    pub fn node_type(&self, id: NodeTypeId) -> Option<&dyn NodeType> {
        self.node_types.get(&id).map(|e| &*e.item)
    }

    /// Conversion `id`.
    pub fn conversion(&self, id: ConversionId) -> Option<&dyn Conversion> {
        self.conversions.get(&id).map(|e| &*e.item)
    }

    /// The conversion from `from` to `to`, when a package provides one.
    pub fn conversion_between(&self, from: DataTypeId, to: DataTypeId) -> Option<&dyn Conversion> {
        self.routes
            .get(&(from, to))
            .and_then(|&id| self.conversion(id))
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

    /// The formats whose files hold values of type `ty`.
    pub fn formats_producing(&self, ty: DataTypeId) -> impl Iterator<Item = &dyn FileFormat> {
        self.formats().filter(move |f| f.produces().contains(&ty))
    }

    /// Session slot `id`.
    pub fn slot(&self, id: SlotId) -> Option<&dyn SessionSlot> {
        self.slots.get(&id).map(|e| &*e.item)
    }

    /// A new session's slots, each at its initial value.
    pub fn new_session(&self) -> SessionSlots {
        SessionSlots::new(self.slots.values().map(|e| &*e.item))
    }

    /// The package that registered data type `id`.
    pub fn data_type_owner(&self, id: DataTypeId) -> Option<PackageId> {
        self.data_types.get(&id).map(|e| e.package)
    }

    /// The package that registered object kind `id`.
    pub fn object_kind_owner(&self, id: ObjectKindId) -> Option<PackageId> {
        self.object_kinds.get(&id).map(|e| e.package)
    }

    /// The package that registered format `id`.
    pub fn format_owner(&self, id: FormatId) -> Option<PackageId> {
        self.formats.get(&id).map(|e| e.package)
    }
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("packages", &self.packages)
            .field("data_types", &self.data_types.keys().collect::<Vec<_>>())
            .field("held_edits", &self.held_edits.keys().collect::<Vec<_>>())
            .field("node_types", &self.node_types.keys().collect::<Vec<_>>())
            .field("conversions", &self.conversions.keys().collect::<Vec<_>>())
            .field(
                "object_kinds",
                &self.object_kinds.keys().collect::<Vec<_>>(),
            )
            .field("formats", &self.formats.keys().collect::<Vec<_>>())
            .field("slots", &self.slots.keys().collect::<Vec<_>>())
            .finish()
    }
}
