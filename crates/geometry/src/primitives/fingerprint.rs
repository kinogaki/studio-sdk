//! The table's fingerprint: BLAKE3 of the point and primitive counts and the
//! digest of every block of [`ROWS`] rows (BLAKE3 of each row's length and
//! corners, little-endian); a table that isn't all polygons hashes that again
//! with its kinds and curve data.
//!
//! The digests are [`Digests`], kept with the table, so a table an edit made
//! hashes only the blocks of the rows it wrote: a row of another length moves
//! the vertices after it, and their rows' digests stay. Blocks fall at fixed
//! rows, so equal tables hash equal whatever made them.
use super::{Kind, Kinds, Primitives};
use crate::parallel;

/// A hash.
type Hash = [u8; 32];

/// The rows a digest covers.
const ROWS: usize = 4096;

/// The digest of every block of a table's rows.
#[derive(Clone, Debug)]
pub(super) struct Digests(Vec<Hash>);

impl Digests {
    /// Every block of `t` hashed.
    fn of(t: &Primitives) -> Self {
        Self::after(t, &[], None)
    }

    /// `t`'s digests: read from `base`, the digests of a table of the same
    /// rows but for `changed` (ascending), for the blocks holding none of
    /// them.
    pub(super) fn after(t: &Primitives, changed: &[u32], base: Option<&Digests>) -> Self {
        let blocks = parallel::map(t.len().div_ceil(ROWS), 1, |blocks| {
            (blocks.map(|b| {
                let first = changed.partition_point(|&row| (row as usize) < b * ROWS);
                let touched = changed
                    .get(first)
                    .is_some_and(|&row| (row as usize) < (b + 1) * ROWS);
                match base.and_then(|base| base.0.get(b)).filter(|_| !touched) {
                    Some(&digest) => digest,
                    None => block(t, b),
                }
            }))
            .collect::<Vec<_>>()
        });
        Self(blocks.into_iter().flatten().collect())
    }
}

/// Block `b`'s digest.
fn block(t: &Primitives, b: usize) -> Hash {
    let mut hash = blake3::Hasher::new();
    for p in b * ROWS..((b + 1) * ROWS).min(t.len()) {
        let row = t.primitive(p);
        hash.update(&(row.len() as u32).to_le_bytes());
        little_endian(&mut hash, row);
    }
    *hash.finalize().as_bytes()
}

pub(super) fn of(t: &Primitives) -> Hash {
    let digests = t.digests.get_or_init(|| Digests::of(t));
    let mut hash = blake3::Hasher::new();
    hash.update(&(t.points as u64).to_le_bytes());
    hash.update(&(t.len() as u64).to_le_bytes());
    for digest in &digests.0 {
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
