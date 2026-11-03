//! The table's fingerprint: BLAKE3 of the little-endian byte stream
//! `points as u64 ++ primitives as u64 ++ offsets ++ vertices`; a table
//! that isn't all polygons hashes that hash again with its kinds and curve
//! data.
//!
//! Large inputs hash fixed-size subtrees in parallel and merge their
//! chaining values in BLAKE3's own tree shape, which gives the hash of one
//! sequential pass (tested against `blake3::hash`).
use super::{Kind, Kinds, Primitives};
use crate::parallel;
use blake3::hazmat::{
    ChainingValue, HasherExt, Mode, merge_subtrees_non_root, merge_subtrees_root,
};

/// Bytes per parallel leaf: a power of two of BLAKE3 chunks (256 chunks).
const LEAF: usize = 256 * blake3::CHUNK_LEN;

pub(super) fn of(t: &Primitives) -> [u8; 32] {
    let rows = rows(t.points, t.len(), &t.offsets, &t.vertices);
    if matches!(t.kinds, Kinds::All(Kind::POLYGON)) && t.curves.is_none() {
        return rows;
    }
    let mut hash = blake3::Hasher::new();
    hash.update(&rows);
    match &t.kinds {
        Kinds::All(kind) => hash.update(&[0, kind.byte()]),
        Kinds::Each(kinds) => hash.update(&[1]).update(bytemuck::cast_slice(kinds)),
    };
    if let Some(c) = &t.curves {
        hash.update(c.degrees());
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

/// The hash of the rows alone.
fn rows(points: usize, primitives: usize, offsets: &[u32], vertices: &[u32]) -> [u8; 32] {
    let mut header = [0u8; 16];
    header[..8].copy_from_slice(&(points as u64).to_le_bytes());
    header[8..].copy_from_slice(&(primitives as u64).to_le_bytes());
    let message = Message {
        header,
        offsets,
        vertices,
    };
    let len = message.len();
    if len <= LEAF {
        let mut hash = blake3::Hasher::new();
        message.hash_into(&mut hash, 0, len);
        return *hash.finalize().as_bytes();
    }
    let leaves = parallel::map(len, LEAF, |range| {
        let mut hash = blake3::Hasher::new();
        hash.set_input_offset(range.start as u64);
        message.hash_into(&mut hash, range.start, range.end);
        hash.finalize_non_root()
    });
    let split = left_leaves(leaves.len());
    let (left, right) = (merge(&leaves[..split]), merge(&leaves[split..]));
    *merge_subtrees_root(&left, &right, Mode::Hash).as_bytes()
}

/// BLAKE3's left subtree holds the largest power of two of chunks below
/// the total; with power-of-two leaves that is the largest power of two
/// of leaves below `count` (count ≥ 2).
fn left_leaves(count: usize) -> usize {
    debug_assert!(count >= 2);
    1 << (usize::BITS - 1 - (count - 1).leading_zeros())
}

fn merge(leaves: &[ChainingValue]) -> ChainingValue {
    if leaves.len() == 1 {
        return leaves[0];
    }
    let split = left_leaves(leaves.len());
    merge_subtrees_non_root(
        &merge(&leaves[..split]),
        &merge(&leaves[split..]),
        Mode::Hash,
    )
}

/// The byte stream, in three segments.
struct Message<'a> {
    header: [u8; 16],
    offsets: &'a [u32],
    vertices: &'a [u32],
}

impl Message<'_> {
    fn len(&self) -> usize {
        16 + 4 * (self.offsets.len() + self.vertices.len())
    }

    /// Feeds bytes `start..end` of the stream to `hash`.
    fn hash_into(&self, hash: &mut blake3::Hasher, start: usize, end: usize) {
        let header_end = 16;
        let offsets_end = header_end + 4 * self.offsets.len();
        if start < header_end {
            hash.update(&self.header[start..end.min(header_end)]);
        }
        words(hash, self.offsets, header_end, start, end);
        words(hash, self.vertices, offsets_end, start, end);
    }
}

/// Feeds the bytes of `values` (starting at stream byte `base`) that fall
/// in stream bytes `start..end`. Every boundary is a multiple of 4.
fn words(hash: &mut blake3::Hasher, values: &[u32], base: usize, start: usize, end: usize) {
    let from = start.max(base);
    let to = end.min(base + 4 * values.len());
    if from >= to {
        return;
    }
    let part = &values[(from - base) / 4..(to - base) / 4];
    little_endian(hash, part);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sequential(points: usize, primitives: usize, offsets: &[u32], vertices: &[u32]) -> [u8; 32] {
        let mut bytes = (points as u64).to_le_bytes().to_vec();
        bytes.extend((primitives as u64).to_le_bytes());
        for v in offsets.iter().chain(vertices) {
            bytes.extend(v.to_le_bytes());
        }
        *blake3::hash(&bytes).as_bytes()
    }

    #[test]
    fn parallel_subtrees_hash_exactly_like_one_pass_at_every_size() {
        // Around one leaf, exact multiples and odd remainders (in words).
        let quarter = LEAF / 4;
        for words in [
            4,
            5,
            200,
            quarter - 5,
            quarter - 4,
            quarter,
            quarter * 2,
            quarter * 3 + 5,
            quarter * 8,
            quarter * 21 + 3,
        ] {
            let offsets: Vec<u32> = (0..words as u32 / 3).collect();
            let vertices: Vec<u32> = (0..(words - offsets.len()) as u32)
                .map(|i| i.wrapping_mul(2_654_435_761))
                .collect();
            let points = (1usize << 40) | 77;
            assert_eq!(
                rows(points, 3, &offsets, &vertices),
                sequential(points, 3, &offsets, &vertices),
                "{words} words"
            );
        }
    }
}
