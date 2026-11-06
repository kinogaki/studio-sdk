//! Scene updates: what the host pushes, in batches, and how batches an engine
//! has not applied yet coalesce into one.
//!
//! - An add and a later remove of the same prim cancel.
//! - A change to a prim added in the same batch replaces the added payload.
//! - Dirty bits of two changes OR together; the later payload wins.
//! - Point ranges merge, and fall back to the whole stream past a cap
//!   (`Ranges::MOST`) or when either change was not a patch.
//!
//! A drag that sends 60 updates a second while a pass is in flight is
//! therefore applied once, at the next pass boundary.

use crate::dirty::Dirty;
use crate::id::PrimId;
use crate::prim::Prim;
use crate::ranges::Ranges;
use crate::settings::SceneSettings;
use std::collections::{HashMap, HashSet};

/// A changed prim: what changed, its new payload, and for a POINTS change
/// the points that moved.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// The prim.
    pub id: PrimId,
    /// What changed.
    pub dirty: Dirty,
    /// The prim as it is now.
    pub prim: Prim,
    /// With POINTS: the points that moved (whole: all of them).
    pub points: Ranges,
}

impl Change {
    /// A change of `dirty` to `prim`; POINTS changes cover every point.
    pub fn new(id: PrimId, dirty: Dirty, prim: Prim) -> Change {
        let points = if dirty.contains(Dirty::POINTS) {
            Ranges::whole()
        } else {
            Ranges::new()
        };
        Change {
            id,
            dirty,
            prim,
            points,
        }
    }

    /// A drag's change: POINTS of `prim` at `points` only.
    pub fn points(id: PrimId, prim: Prim, points: Ranges) -> Change {
        Change {
            id,
            dirty: Dirty::POINTS,
            prim,
            points,
        }
    }

    /// `later` folded into this change of the same prim.
    fn absorb(&mut self, later: Change) {
        let points = self.dirty.contains(Dirty::POINTS);
        let later_points = later.dirty.contains(Dirty::POINTS);
        match (points, later_points) {
            (true, true) => self.points.merge(&later.points),
            (false, true) => self.points = later.points,
            _ => {}
        }
        self.dirty |= later.dirty;
        self.prim = later.prim;
    }
}

/// One batch of scene edits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneUpdate {
    /// New prims.
    pub added: Vec<(PrimId, Prim)>,
    /// Prims gone.
    pub removed: Vec<PrimId>,
    /// Prims changed.
    pub changed: Vec<Change>,
    /// New scene settings, if they changed.
    pub settings: Option<SceneSettings>,
    /// A drag ended: meshes refitted during it may now be rebuilt.
    pub settled: bool,
}

impl SceneUpdate {
    /// An empty batch.
    pub fn new() -> SceneUpdate {
        SceneUpdate::default()
    }

    /// Whether the batch changes nothing.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.settings.is_none()
            && !self.settled
    }

    /// Fold `later` (sent after this batch) into this batch, so applying the
    /// result equals applying both in order. Within `later`, a removal wins
    /// over a change of the same prim. Linear in both batches: each prim is
    /// found by its id, and the lists keep their order (what a prim names is
    /// sent before it).
    pub fn merge(&mut self, later: SceneUpdate) {
        let later_drags = later
            .changed
            .iter()
            .any(|c| c.dirty.contains(Dirty::POINTS));
        let mut added = index(self.added.iter().map(|(id, _)| *id));
        let mut changed = index(self.changed.iter().map(|c| c.id));
        for (id, prim) in later.added {
            added.entry(id).or_insert(self.added.len());
            self.added.push((id, prim));
        }
        for change in later.changed {
            if let Some(&at) = added.get(&change.id) {
                self.added[at].1 = change.prim;
            } else if let Some(&at) = changed.get(&change.id) {
                self.changed[at].absorb(change);
            } else {
                changed.insert(change.id, self.changed.len());
                self.changed.push(change);
            }
        }
        let mut removed: HashSet<PrimId> = self.removed.iter().copied().collect();
        let (mut gone, mut cancelled) = (HashSet::new(), HashSet::new());
        for id in later.removed {
            gone.insert(id);
            if added.remove(&id).is_some() {
                // Added and removed before an engine saw it: nothing happened.
                cancelled.insert(id);
            } else if removed.insert(id) {
                self.removed.push(id);
            }
        }
        if !gone.is_empty() {
            self.added.retain(|(id, _)| !cancelled.contains(id));
            self.changed.retain(|c| !gone.contains(&c.id));
        }
        if later.settings.is_some() {
            self.settings = later.settings;
        }
        // A later drag step means the drag goes on; otherwise a settle stands.
        self.settled = if later_drags {
            later.settled
        } else {
            self.settled || later.settled
        };
    }
}

/// Each id's first place in a list of them.
fn index(ids: impl Iterator<Item = PrimId>) -> HashMap<PrimId, usize> {
    let mut at = HashMap::new();
    for (i, id) in ids.enumerate() {
        at.entry(id).or_insert(i);
    }
    at
}
