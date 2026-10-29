//! [`RefFilter`]: what a reference parameter may name. The parameter says
//! what fits; the host lists the scene's objects, nodes and Materials that
//! do, lets the user pick one (from a searchable list, or dragged from the
//! outliner or a network) and resolves the path, written from the root or
//! from the node holding the parameter, when the node cooks.
use crate::id::{DataTypeId, ObjectKindId};

/// What a reference may name, and whether it may name several at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefFilter {
    /// What fits.
    pub fits: Fits,
    /// The path may hold wildcards (`*`, `?`) and name every match.
    pub patterns: bool,
}

/// What fits a reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fits {
    /// Objects of these kinds (cameras: the camera object kind); any object
    /// when empty.
    Objects(Vec<ObjectKindId>),
    /// Objects and nodes whose result is of one of these data types
    /// (geometry: a Mesh object, a Part, a node in a Mesh network).
    Data(Vec<DataTypeId>),
    /// Any Material: one of a Material Library's, or a slot of a printer.
    Materials,
    /// A printer's Material slots only.
    MaterialSlots,
}

impl RefFilter {
    /// One object of `kinds` (any object when empty).
    pub fn objects(kinds: impl IntoIterator<Item = ObjectKindId>) -> Self {
        Self::of(Fits::Objects(kinds.into_iter().collect()))
    }

    /// One object or node whose result is of `types`.
    pub fn data(types: impl IntoIterator<Item = DataTypeId>) -> Self {
        Self::of(Fits::Data(types.into_iter().collect()))
    }

    /// One Material: any, or with `slots_only` a printer's slot.
    pub fn material(slots_only: bool) -> Self {
        Self::of(if slots_only {
            Fits::MaterialSlots
        } else {
            Fits::Materials
        })
    }

    /// Naming every match of a pattern too.
    pub fn patterns(mut self) -> Self {
        self.patterns = true;
        self
    }

    fn of(fits: Fits) -> Self {
        RefFilter {
            fits,
            patterns: false,
        }
    }
}
