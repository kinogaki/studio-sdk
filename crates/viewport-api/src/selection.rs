//! What a surface shows selected: at most one [`ElementSet`] for each
//! family of drawn elements, so levels that combine (a sketch's curves and
//! points, CVs beside hull lines) show their elements selected together.
use crate::{ElementKind, ElementSet};

/// The selection shown on the edited surface: faces (or whole objects),
/// lines and markers, each in its own kind's selected look. A present
/// slot is a level the surface shows, even with nothing in it: a present
/// `points` draws the markers, a present `edges` the wires (as a single
/// set of that kind does). Most surfaces show one kind
/// ([`ElementSelection::one`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ElementSelection<'a> {
    /// [`ElementKind::Face`], or [`ElementKind::Object`] alone (object
    /// selection draws silhouettes and no component feedback).
    pub faces: Option<&'a ElementSet>,
    /// [`ElementKind::Edge`]: line ids of any class.
    pub edges: Option<&'a ElementSet>,
    /// [`ElementKind::Point`] markers or [`ElementKind::Vertex`] records.
    pub points: Option<&'a ElementSet>,
}

impl<'a> ElementSelection<'a> {
    /// `set` alone, in its kind's slot.
    pub fn one(set: &'a ElementSet) -> Self {
        Self::default().with(set)
    }

    /// This selection with `set` in its kind's slot (replacing what was there).
    pub fn with(mut self, set: &'a ElementSet) -> Self {
        *self.slot_mut(Slot::of(set.kind)) = Some(set);
        self
    }

    /// The sets present, faces first.
    pub fn sets(&self) -> impl Iterator<Item = &'a ElementSet> {
        [self.faces, self.edges, self.points].into_iter().flatten()
    }

    /// Whether whole objects are selected (the faces slot holds objects).
    pub fn objects(&self) -> bool {
        self.faces.is_some_and(|s| s.kind == ElementKind::Object)
    }

    /// Whether nothing is selected in any slot.
    pub fn is_empty(&self) -> bool {
        self.sets().all(ElementSet::is_empty)
    }

    /// Why this selection can't be shown: a set in another kind's slot, or
    /// whole objects beside components.
    pub fn validate(&self) -> Result<(), String> {
        for (slot, set) in [
            (Slot::Faces, self.faces),
            (Slot::Edges, self.edges),
            (Slot::Points, self.points),
        ] {
            if let Some(set) = set
                && Slot::of(set.kind) != slot
            {
                return Err(format!("a {:?} selection in the {slot:?} slot", set.kind));
            }
        }
        if self.objects() && (self.edges.is_some() || self.points.is_some()) {
            return Err("an object selection shows alone".into());
        }
        Ok(())
    }

    fn slot_mut(&mut self, slot: Slot) -> &mut Option<&'a ElementSet> {
        match slot {
            Slot::Faces => &mut self.faces,
            Slot::Edges => &mut self.edges,
            Slot::Points => &mut self.points,
        }
    }
}

/// The slot a kind's set goes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Faces,
    Edges,
    Points,
}

impl Slot {
    fn of(kind: ElementKind) -> Self {
        match kind {
            ElementKind::Face | ElementKind::Object => Self::Faces,
            ElementKind::Edge => Self::Edges,
            ElementKind::Point | ElementKind::Vertex => Self::Points,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_go_to_their_kinds_slots() {
        let faces = ElementSet::new(ElementKind::Face, false, [1]);
        let edges = ElementSet::new(ElementKind::Edge, false, [2]);
        let points = ElementSet::new(ElementKind::Vertex, false, []);
        let s = ElementSelection::one(&edges).with(&points).with(&faces);
        assert_eq!(s.faces, Some(&faces));
        assert_eq!(s.edges, Some(&edges));
        assert_eq!(s.points, Some(&points));
        assert_eq!(s.sets().count(), 3);
        assert!(!s.is_empty() && !s.objects());
        assert!(s.validate().is_ok());
        // An empty slot is still a level shown, with nothing selected.
        assert!(ElementSelection::one(&points).is_empty());
        assert!(ElementSelection::default().is_empty());
    }

    #[test]
    fn misplaced_sets_and_objects_beside_components_are_refused() {
        let edges = ElementSet::new(ElementKind::Edge, false, [2]);
        let objects = ElementSet::new(ElementKind::Object, false, [0]);
        let misplaced = ElementSelection {
            points: Some(&edges),
            ..Default::default()
        };
        assert!(misplaced.validate().is_err());
        assert!(ElementSelection::one(&objects).validate().is_ok());
        assert!(ElementSelection::one(&objects).objects());
        assert!(
            ElementSelection::one(&objects)
                .with(&edges)
                .validate()
                .is_err()
        );
    }
}
