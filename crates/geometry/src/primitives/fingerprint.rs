//! The table's fingerprint: BLAKE3 of the point and primitive counts and the
//! digest of every cell of the offsets and the vertices (BLAKE3 of the
//! cell's little-endian bytes); a table that isn't all polygons hashes that
//! again with its kinds and curve data.
//!
//! The digests are [`Digests`], kept with the table, so a table an edit made
//! hashes only the cells it doesn't share with the table it came from. Cells
//! fall at fixed places, so equal tables hash equal whatever made them.
use super::{Kind, Kinds, Primitives};
use crate::column::Column;
use crate::parallel;

/// A hash.
type Hash = [u8; 32];

/// The digest of every cell of a table's two row columns.
#[derive(Clone, Debug)]
pub(super) struct Digests {
    offsets: Vec<Hash>,
    vertices: Vec<Hash>,
}

impl Digests {
    /// Every cell of `t` hashed.
    fn of(t: &Primitives) -> Self {
        Self {
            offsets: digests(&t.offsets, None),
            vertices: digests(&t.vertices, None),
        }
    }

    /// `t`'s digests, read from `base`'s (those of `of`) for the cells they
    /// share.
    pub(super) fn after(t: &Primitives, of: &Primitives, base: &Digests) -> Self {
        Self {
            offsets: digests(&t.offsets, Some((&of.offsets, &base.offsets))),
            vertices: digests(&t.vertices, Some((&of.vertices, &base.vertices))),
        }
    }
}

/// `column`'s cell digests; a cell shared with `base`'s column is its digest.
fn digests(column: &Column<u32>, base: Option<(&Column<u32>, &Vec<Hash>)>) -> Vec<Hash> {
    let blocks = parallel::map(column.cell_count(), 1, |cells| {
        cells
            .map(|c| match base {
                Some((from, hashes)) if column.shares_cell(from, c) => hashes[c],
                _ => {
                    let mut hash = blake3::Hasher::new();
                    little_endian(&mut hash, column.cell(c));
                    *hash.finalize().as_bytes()
                }
            })
            .collect::<Vec<_>>()
    });
    blocks.into_iter().flatten().collect()
}

pub(super) fn of(t: &Primitives) -> Hash {
    let digests = t.digests.get_or_init(|| Digests::of(t));
    let mut hash = blake3::Hasher::new();
    hash.update(&(t.points as u64).to_le_bytes());
    hash.update(&(t.len() as u64).to_le_bytes());
    for digest in digests.offsets.iter().chain(&digests.vertices) {
        hash.update(digest);
    }
    match &t.kinds {
        Kinds::All(Kind::POLYGON) => {}
        Kinds::All(kind) => {
            hash.update(&[0, kind.byte()]);
        }
        Kinds::Each(kinds) => {
            hash.update(&[1]);
            for run in kinds.runs() {
                hash.update(bytemuck::cast_slice(run));
            }
        }
    }
    if let Some(c) = &t.curves {
        hash.update(c.degrees().as_slice());
        let knots = c.knots();
        for o in knots.offsets().iter() {
            hash.update(&o.to_le_bytes());
        }
        for k in knots
            .values()
            .iter()
            .chain(c.weights().into_iter().flatten())
        {
            hash.update(&k.to_le_bytes());
        }
        hash.update(&[u8::from(c.weights().is_some())]);
    }
    *hash.finalize().as_bytes()
}

/// `values` as little-endian bytes, without copying them on a
/// little-endian machine.
#[cfg(target_endian = "little")]
fn little_endian(hash: &mut blake3::Hasher, values: &[u32]) {
    hash.update(bytemuck::cast_slice(values));
}

#[cfg(target_endian = "big")]
fn little_endian(hash: &mut blake3::Hasher, values: &[u32]) {
    for chunk in values.chunks(4096) {
        let bytes: Vec<u8> = chunk.iter().flat_map(|v| v.to_le_bytes()).collect();
        hash.update(&bytes);
    }
}
