//! [`Selection`]: which elements of one domain of a standard value are
//! selected — the standard component selection.
use crate::attributes::Domain;
use crate::column::Column;
use crate::elements::Elements;
use crate::error::{ErrorKind, GeometryError};
use crate::incidence::Incidence;

/// Selected elements of one domain ([`Elements`]); the active one (the
/// last picked, which tools that need one element start from); the
/// fingerprint of the value it was made on; and, when that value carries
/// stable ids, the members' ids.
///
/// **Rules** (each constructor checks them): the active element is a
/// member; ids, when held, are one per member, in member order.
///
/// **Staleness.** Once the value's [fingerprint](Incidence::fingerprint)
/// changes, the members' numbers mean other elements
/// ([`validate`](Self::validate) refuses it). A selection holding ids can
/// be found again on the new value (`kinogaki-edit`'s `reresolve`); one
/// without is reset by its holder.
///
/// ```
/// use kinogaki_geometry::{Domain, Elements, Selection};
///
/// let members = Elements::from_indices(10, [7, 3, 7, 1]).unwrap();
/// let s = Selection::new(Domain::Point, members, Some(3)).unwrap();
/// assert_eq!(s.members().iter().collect::<Vec<_>>(), [1, 3, 7]);
/// assert!(s.contains(7) && !s.contains(2));
/// let other = Elements::from_indices(10, [1]).unwrap();
/// assert!(Selection::new(Domain::Point, other, Some(2)).is_err());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    domain: Domain,
    members: Elements,
    active: Option<u32>,
    on: Option<[u8; 32]>,
    ids: Option<Column<i64>>,
}

impl Selection {
    /// `members` of `domain`, `active` among them, made on no value in
    /// particular.
    ///
    /// # Errors
    /// When `active` is not a member.
    pub fn new(
        domain: Domain,
        members: Elements,
        active: Option<u32>,
    ) -> Result<Self, GeometryError> {
        if let Some(a) = active
            && !members.contains(a)
        {
            return Err(GeometryError::at(
                ErrorKind::Selection,
                a as usize,
                format!("the active {} {a} is not selected", domain.name()),
            ));
        }
        Ok(Self {
            domain,
            members,
            active,
            on: None,
            ids: None,
        })
    }

    /// `members` of `domain` of `value`, `active` among them: it keeps
    /// the value's fingerprint and, when the value has stable ids there,
    /// the members' ids.
    ///
    /// # Errors
    /// When `active` is not a member, `value` has no `domain`, or
    /// `members` is a set of another count.
    pub fn on(
        value: &dyn Incidence,
        domain: Domain,
        members: Elements,
        active: Option<u32>,
    ) -> Result<Self, GeometryError> {
        let mut s = Self::new(domain, members, active)?;
        s.check_count(value)?;
        s.on = Some(value.fingerprint());
        s.ids = value.ids(domain).map(|ids| match s.members.is_all() {
            true => ids.clone(),
            false => s.members.iter().map(|i| ids[i as usize]).collect(),
        });
        Ok(s)
    }

    /// Nothing of the `count` elements of `domain`.
    pub fn none(domain: Domain, count: usize) -> Self {
        Self::new(domain, Elements::none(count), None).expect("nothing has no active element")
    }

    /// This selection as saved: made on the value whose fingerprint is
    /// `on`, its members' ids `ids` (what [`on`](Self::on) kept).
    ///
    /// # Errors
    /// Ids that aren't one per member.
    pub fn restored(
        mut self,
        on: Option<[u8; 32]>,
        ids: Option<Column<i64>>,
    ) -> Result<Self, GeometryError> {
        if let Some(ids) = &ids
            && ids.len() != self.members.len()
        {
            return Err(GeometryError::whole(
                ErrorKind::Selection,
                format!(
                    "{} ids for {} selected {} elements",
                    ids.len(),
                    self.members.len(),
                    self.domain.name()
                ),
            ));
        }
        self.on = on;
        self.ids = ids;
        Ok(self)
    }

    /// The domain selected.
    pub fn domain(&self) -> Domain {
        self.domain
    }

    /// The selected elements.
    pub fn members(&self) -> &Elements {
        &self.members
    }

    /// The active element.
    pub fn active(&self) -> Option<u32> {
        self.active
    }

    /// The fingerprint of the value it was made on (`None`: on no value
    /// in particular).
    pub fn made_on(&self) -> Option<[u8; 32]> {
        self.on
    }

    /// The members' stable ids, in member order, when the value it was
    /// made on carried them.
    pub fn ids(&self) -> Option<&Column<i64>> {
        self.ids.as_ref()
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Whether element `i` is selected.
    pub fn contains(&self, i: u32) -> bool {
        self.members.contains(i)
    }

    /// How many are selected.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether it was made on `on` (or on no value in particular) and is
    /// a set of as many elements as `on` has of its domain.
    ///
    /// # Errors
    /// A stale selection ([`ErrorKind::Selection`], no element), a domain
    /// `on` has none of, or a set of another count.
    pub fn validate(&self, on: &dyn Incidence) -> Result<(), GeometryError> {
        if self.on.is_some_and(|t| t != on.fingerprint()) {
            return Err(GeometryError::whole(
                ErrorKind::Selection,
                "the selection was made on another value",
            ));
        }
        self.check_count(on)
    }

    fn check_count(&self, on: &dyn Incidence) -> Result<(), GeometryError> {
        let Some(count) = on.count(self.domain) else {
            return Err(GeometryError::whole(
                ErrorKind::Selection,
                format!("the value has no {} domain", self.domain.name()),
            ));
        };
        if self.members.count() != count {
            return Err(GeometryError::whole(
                ErrorKind::Selection,
                format!(
                    "a selection of {} {} elements on a value of {count}",
                    self.members.count(),
                    self.domain.name()
                ),
            ));
        }
        Ok(())
    }
}
