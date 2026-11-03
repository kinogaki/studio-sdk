//! [`Selection`]: which elements of one domain of a standard value are
//! selected — the standard component selection.
use crate::attributes::Domain;
use crate::column::Column;
use crate::error::{ErrorKind, GeometryError};
use crate::incidence::Incidence;

/// Selected elements of one domain: all of them, or a sorted set of
/// indices; the active one (the last picked, which tools that need one
/// element start from); and the topology it was made on.
///
/// **Rules** (`Selection::from_ascending` checks them): indices are
/// strictly ascending ([`Selection::new`] sorts and deduplicates them);
/// the active element is one of the selected.
///
/// **Staleness.** A selection made on a value keeps its
/// [fingerprint](Incidence::fingerprint); once that changes the indices
/// mean other elements, so the holder resets it
/// ([`validate`](Self::validate) refuses it).
///
/// ```
/// use kinogaki_geometry::{Domain, Selection};
///
/// let s = Selection::new(Domain::Point, vec![7, 3, 7, 1], Some(3), None).unwrap();
/// assert_eq!(&**s.indices(), &[1, 3, 7]);
/// assert!(s.contains(7) && !s.contains(2));
/// assert!(Selection::new(Domain::Point, vec![1], Some(2), None).is_err());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    domain: Domain,
    all: bool,
    indices: Column<u32>,
    active: Option<u32>,
    topology: Option<[u8; 32]>,
}

impl Selection {
    /// The elements `indices` of `domain` (sorted and deduplicated here),
    /// `active` among them, made on the topology whose fingerprint is
    /// `topology` (`None`: on no particular one).
    ///
    /// # Errors
    /// When `active` is not selected.
    pub fn new(
        domain: Domain,
        mut indices: Vec<u32>,
        active: Option<u32>,
        topology: Option<[u8; 32]>,
    ) -> Result<Self, GeometryError> {
        indices.sort_unstable();
        indices.dedup();
        Self::from_ascending(domain, Column::new(indices), active, topology)
    }

    /// The elements `indices` of `domain`, already strictly ascending, kept
    /// as given: a selection made from another's
    /// [`indices`](Self::indices) shares them.
    ///
    /// # Errors
    /// When `indices` are not strictly ascending, or `active` is not
    /// selected.
    fn from_ascending(
        domain: Domain,
        indices: Column<u32>,
        active: Option<u32>,
        topology: Option<[u8; 32]>,
    ) -> Result<Self, GeometryError> {
        if let Some(i) = indices.windows(2).position(|w| w[0] >= w[1]) {
            return Err(GeometryError::at(
                ErrorKind::Selection,
                i + 1,
                format!(
                    "the selected {} indices are not strictly ascending at {}",
                    domain.name(),
                    i + 1
                ),
            ));
        }
        if let Some(a) = active
            && indices.binary_search(&a).is_err()
        {
            return Err(GeometryError::at(
                ErrorKind::Selection,
                a as usize,
                format!("the active {} {a} is not selected", domain.name()),
            ));
        }
        Ok(Self {
            domain,
            all: false,
            indices,
            active,
            topology,
        })
    }

    /// Every element of `domain`, made on the topology whose fingerprint is
    /// `topology`; none active.
    pub fn all(domain: Domain, topology: Option<[u8; 32]>) -> Self {
        Self {
            domain,
            all: true,
            indices: Column::default(),
            active: None,
            topology,
        }
    }

    /// Nothing of `domain`.
    pub fn none(domain: Domain) -> Self {
        Self {
            domain,
            all: false,
            indices: Column::default(),
            active: None,
            topology: None,
        }
    }

    /// The domain selected.
    pub fn domain(&self) -> Domain {
        self.domain
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

    /// Whether it was made on `on` (or on no value in particular) and
    /// every index is one of its elements.
    ///
    /// # Errors
    /// A stale selection ([`ErrorKind::Selection`], no element), a domain
    /// `on` has none of, or the first index past the end (at it).
    pub fn validate(&self, on: &dyn Incidence) -> Result<(), GeometryError> {
        if self.topology.is_some_and(|t| t != on.fingerprint()) {
            return Err(GeometryError::whole(
                ErrorKind::Selection,
                "the selection was made on another value",
            ));
        }
        let Some(count) = on.count(self.domain) else {
            return Err(GeometryError::whole(
                ErrorKind::Selection,
                format!("the value has no {} domain", self.domain.name()),
            ));
        };
        match self.indices.last() {
            Some(&last) if last as usize >= count => {
                let first = self.indices.partition_point(|&i| (i as usize) < count);
                let i = self.indices[first];
                Err(GeometryError::at(
                    ErrorKind::Selection,
                    i as usize,
                    format!("{} {i} is past the last of {count}", self.domain.name()),
                ))
            }
            _ => Ok(()),
        }
    }

    /// How many are selected out of `count` elements.
    pub fn len(&self, count: usize) -> usize {
        if self.all { count } else { self.indices.len() }
    }
}
