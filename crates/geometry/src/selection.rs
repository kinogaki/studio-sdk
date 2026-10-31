//! [`Selection`]: which elements of one class of a standard value are
//! selected — the standard component selection.
use crate::attributes::Class;
use crate::column::Column;
use crate::error::{ErrorKind, GeometryError};
use crate::topology::Topology;

/// Selected elements of one class: all of them, or a sorted set of
/// indices; the active one (the last picked, which tools that need one
/// element start from); and the topology it was made on.
///
/// **Rules** ([`Selection::new`] checks them): indices are sorted and
/// distinct (the constructor sorts and deduplicates them); the active
/// element is one of the selected.
///
/// **Staleness.** A selection made on a topology keeps its
/// [fingerprint](Topology::fingerprint); once the topology changes the
/// indices mean other elements, so the holder resets it
/// ([`is_stale`](Self::is_stale)). A Piece selection selects primitives
/// by their indices, as Primitive does.
///
/// ```
/// use kinogaki_geometry::{Class, Selection};
///
/// let s = Selection::new(Class::Point, vec![7, 3, 7, 1], Some(3), None).unwrap();
/// assert_eq!(&**s.indices(), &[1, 3, 7]);
/// assert!(s.contains(7) && !s.contains(2));
/// assert!(Selection::new(Class::Point, vec![1], Some(2), None).is_err());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    class: Class,
    all: bool,
    indices: Column<u32>,
    active: Option<u32>,
    topology: Option<[u8; 32]>,
}

impl Selection {
    /// The elements `indices` of `class` (sorted and deduplicated here),
    /// `active` among them, made on the topology whose fingerprint is
    /// `topology` (`None`: on no particular one).
    ///
    /// # Errors
    /// When `active` is not selected.
    pub fn new(
        class: Class,
        mut indices: Vec<u32>,
        active: Option<u32>,
        topology: Option<[u8; 32]>,
    ) -> Result<Self, GeometryError> {
        indices.sort_unstable();
        indices.dedup();
        if let Some(a) = active
            && indices.binary_search(&a).is_err()
        {
            return Err(GeometryError::at(
                ErrorKind::Selection,
                a as usize,
                format!("the active {} {a} is not selected", class.name()),
            ));
        }
        Ok(Self {
            class,
            all: false,
            indices: Column::new(indices),
            active,
            topology,
        })
    }

    /// Every element of `class`, made on the topology whose fingerprint is
    /// `topology`; none active.
    pub fn all(class: Class, topology: Option<[u8; 32]>) -> Self {
        Self {
            class,
            all: true,
            indices: Column::default(),
            active: None,
            topology,
        }
    }

    /// Nothing of `class`.
    pub fn none(class: Class) -> Self {
        Self {
            class,
            all: false,
            indices: Column::default(),
            active: None,
            topology: None,
        }
    }

    /// The class selected.
    pub fn class(&self) -> Class {
        self.class
    }

    /// Whether every element is selected (then [`indices`](Self::indices)
    /// is empty).
    pub fn is_all(&self) -> bool {
        self.all
    }

    /// The selected elements, ascending (empty when [`is_all`](Self::is_all)).
    pub fn indices(&self) -> &Column<u32> {
        &self.indices
    }

    /// The active element.
    pub fn active(&self) -> Option<u32> {
        self.active
    }

    /// The fingerprint of the topology it was made on.
    pub fn topology(&self) -> Option<[u8; 32]> {
        self.topology
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        !self.all && self.indices.is_empty()
    }

    /// Whether element `i` is selected.
    pub fn contains(&self, i: u32) -> bool {
        self.all || self.indices.binary_search(&i).is_ok()
    }

    /// Whether it was made on another topology than `on`: its indices no
    /// longer mean what they did, and its holder resets it. A selection
    /// made on no particular topology is never stale.
    pub fn is_stale(&self, on: &Topology) -> bool {
        self.topology.is_some_and(|t| t != on.fingerprint())
    }

    /// How many are selected out of `count` elements.
    pub fn len(&self, count: usize) -> usize {
        if self.all { count } else { self.indices.len() }
    }
}
