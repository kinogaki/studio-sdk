//! Scene updates: what the host pushes, in batches, and how batches an engine
//! has not applied yet coalesce into one.
//!
//! - An add and a later remove of the same prim cancel.
//! - A change to a prim added in the same batch replaces the added payload.
//! - Dirty bits of two changes OR together; the later payload wins.
//! - Point ranges merge, and fall back to the whole stream past a cap
//!   ([`Ranges::MOST`]) or when either change was not a patch.
//!
//! A drag that sends 60 updates a second while a pass is in flight is
//! therefore applied once, at the next pass boundary.

use crate::dirty::Dirty;
use crate::id::PrimId;
use crate::prim::Prim;
use crate::ranges::Ranges;
use crate::settings::RenderSettings;

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
    /// New render settings (the scene's), if they changed.
    pub settings: Option<RenderSettings>,
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
    /// over a change of the same prim.
    pub fn merge(&mut self, later: SceneUpdate) {
        let later_drags = later
            .changed
            .iter()
            .any(|c| c.dirty.contains(Dirty::POINTS));
        for (id, prim) in later.added {
            self.added.push((id, prim));
        }
        for change in later.changed {
            if let Some(added) = self.added.iter_mut().find(|(a, _)| *a == change.id) {
                added.1 = change.prim;
            } else if let Some(held) = self.changed.iter_mut().find(|c| c.id == change.id) {
                held.absorb(change);
            } else {
                self.changed.push(change);
            }
        }
        for id in later.removed {
            if let Some(at) = self.added.iter().position(|(a, _)| *a == id) {
                // Added and removed before an engine saw it: nothing happened.
                self.added.remove(at);
                self.changed.retain(|c| c.id != id);
                continue;
            }
            self.changed.retain(|c| c.id != id);
            if !self.removed.contains(&id) {
                self.removed.push(id);
            }
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
