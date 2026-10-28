//! Prim ids: a slot and a generation, so an id that was removed never names
//! the prim that later reuses its slot.

/// One prim of a scene: its slot and the generation of that slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrimId {
    /// The slot, dense from 0; engines index their per-prim tables by it.
    pub slot: u32,
    /// How many times the slot was given out before this id.
    pub generation: u32,
}

/// A Material prim's id (materials are referenced by id, never by path).
pub type MaterialId = PrimId;
/// A Texture prim's id.
pub type TextureId = PrimId;

/// Gives out [`PrimId`]s. A released slot is reused, always with the next
/// generation.
#[derive(Clone, Debug, Default)]
pub struct IdAllocator {
    /// Each slot's current generation.
    generations: Vec<u32>,
    /// Whether each slot is in use.
    live: Vec<bool>,
    /// Released slots, reused last-released first.
    free: Vec<u32>,
}

impl IdAllocator {
    /// An allocator with no ids given out.
    pub fn new() -> IdAllocator {
        IdAllocator::default()
    }

    /// A new id: a released slot at its next generation, or a new slot.
    pub fn allocate(&mut self) -> PrimId {
        if let Some(slot) = self.free.pop() {
            let at = slot as usize;
            self.generations[at] = self.generations[at].wrapping_add(1);
            self.live[at] = true;
            return PrimId {
                slot,
                generation: self.generations[at],
            };
        }
        let slot = u32::try_from(self.generations.len()).expect("fewer than 2^32 prims");
        self.generations.push(0);
        self.live.push(true);
        PrimId {
            slot,
            generation: 0,
        }
    }

    /// Whether `id` is live (given out and not released).
    pub fn is_live(&self, id: PrimId) -> bool {
        let at = id.slot as usize;
        at < self.live.len() && self.live[at] && self.generations[at] == id.generation
    }

    /// Release `id`; false (and nothing changes) when it is not live.
    pub fn release(&mut self, id: PrimId) -> bool {
        if !self.is_live(id) {
            return false;
        }
        self.live[id.slot as usize] = false;
        self.free.push(id.slot);
        true
    }

    /// One past the highest slot given out: the size of per-slot tables.
    pub fn slots(&self) -> usize {
        self.generations.len()
    }
}
