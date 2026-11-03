//! Prim ids: a slot and a generation, so an id that was removed never names
//! the prim that later reuses its slot; and engine ids.
use std::fmt;
use std::sync::Arc;

/// A render engine's id (`kinogaki.pixel.engine`), what files and render
/// settings name it by: `<vendor>.<package>.<name>`, as every declared id
/// is, under the vendor of the package describing it. The registry checks
/// both when it admits that package, so an engine a host lists always has
/// a well-formed id of its own vendor; text read from a file is only
/// compared with those.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EngineId(Arc<str>);

impl EngineId {
    /// The engine id `text`.
    pub fn new(text: &str) -> EngineId {
        EngineId(Arc::from(text))
    }

    /// Its text (`kinogaki.pixel.engine`).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for EngineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", &*self.0)
    }
}

impl fmt::Display for EngineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

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
    pub(crate) fn is_live(&self, id: PrimId) -> bool {
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
