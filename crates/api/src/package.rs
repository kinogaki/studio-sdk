//! [`Package`]: what one package registers with the environment.
use crate::conversion::Conversion;
use crate::cook::NodeType;
use crate::data_type::DataType;
use crate::format::FileFormat;
use crate::held::HeldEdit;
use crate::id::PackageId;
use crate::object::ObjectKindSpec;
use crate::session::SessionSlot;
use std::fmt;
use std::sync::Arc;

/// Everything one package contributes through the headless API: its data
/// types (a kernel's), held edits over them, conversions (a bridge's),
/// node types' cooks, object kinds, file formats and session slots. Node
/// descriptions, contexts and UI register through the librarian and the
/// host UI interface.
///
/// Every id it declares has its vendor (the first segment of its id):
/// `acme.trees` registers `acme.trees.branch` or `acme.format.tree`, never
/// `kinogaki.*`.
///
/// ```
/// use kinogaki_studio_api::{Package, PackageId};
/// let package = Package::new(PackageId::declared("acme.trees"), "Trees");
/// assert!(package.data_types.is_empty());
/// ```
#[derive(Clone)]
pub struct Package {
    /// Its id (`kinogaki.mesh`).
    pub id: PackageId,
    /// What a UI calls it ("Mesh").
    pub label: String,
    /// The data types it defines.
    pub data_types: Vec<Arc<dyn DataType>>,
    /// How authored deltas apply to its data types.
    pub held_edits: Vec<Arc<dyn HeldEdit>>,
    /// The conversions it provides between registered data types.
    pub conversions: Vec<Arc<dyn Conversion>>,
    /// What its node types compute.
    pub node_types: Vec<Arc<dyn NodeType>>,
    /// The object kinds it contributes.
    pub object_kinds: Vec<Arc<dyn ObjectKindSpec>>,
    /// The file formats it reads and writes.
    pub formats: Vec<Arc<dyn FileFormat>>,
    /// Its session state.
    pub slots: Vec<Arc<dyn SessionSlot>>,
}

impl Package {
    /// Package `id` called `label`, contributing nothing yet.
    pub fn new(id: PackageId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            data_types: Vec::new(),
            held_edits: Vec::new(),
            conversions: Vec::new(),
            node_types: Vec::new(),
            object_kinds: Vec::new(),
            formats: Vec::new(),
            slots: Vec::new(),
        }
    }

    /// With data type `ty` too.
    pub fn with_data_type(mut self, ty: impl DataType + 'static) -> Self {
        self.data_types.push(Arc::new(ty));
        self
    }

    /// With held edit `held` too.
    pub fn with_held_edit(mut self, held: impl HeldEdit + 'static) -> Self {
        self.held_edits.push(Arc::new(held));
        self
    }

    /// With node type `node` too.
    pub fn with_node_type(mut self, node: impl NodeType + 'static) -> Self {
        self.node_types.push(Arc::new(node));
        self
    }

    /// With conversion `conversion` too.
    pub fn with_conversion(mut self, conversion: impl Conversion + 'static) -> Self {
        self.conversions.push(Arc::new(conversion));
        self
    }

    /// With object kind `kind` too.
    pub fn with_object_kind(mut self, kind: impl ObjectKindSpec + 'static) -> Self {
        self.object_kinds.push(Arc::new(kind));
        self
    }

    /// With file format `format` too.
    pub fn with_format(mut self, format: impl FileFormat + 'static) -> Self {
        self.formats.push(Arc::new(format));
        self
    }

    /// With session slot `slot` too.
    pub fn with_slot(mut self, slot: impl SessionSlot + 'static) -> Self {
        self.slots.push(Arc::new(slot));
        self
    }
}

impl fmt::Debug for Package {
    /// The package by its id and the ids of what it contributes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn ids<T: ?Sized>(items: &[Arc<T>], id: impl Fn(&T) -> &'static str) -> Vec<&'static str> {
            items.iter().map(|item| id(item)).collect()
        }
        f.debug_struct("Package")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("data_types", &ids(&self.data_types, |t| t.id().as_str()))
            .field("held_edits", &ids(&self.held_edits, |h| h.base().as_str()))
            .field("conversions", &ids(&self.conversions, |c| c.id().as_str()))
            .field("node_types", &ids(&self.node_types, |n| n.id().as_str()))
            .field(
                "object_kinds",
                &ids(&self.object_kinds, |k| k.id().as_str()),
            )
            .field("formats", &ids(&self.formats, |f| f.id().as_str()))
            .field("slots", &ids(&self.slots, |s| s.id().as_str()))
            .finish()
    }
}
