//! The topology fingerprint: BLAKE3 of the little-endian stream of the
//! counts (vertices, edges, coedges, loops, faces, shells, lumps, as
//! `u64`), then each table in that order, record by record, field by
//! field as declared: ids as `u32` (`u32::MAX` for none), senses and
//! flags as one byte (`Same` and `false` 0), each precision as a byte (0
//! none, 1 some) and its `f64` bits; then the shells' faces and the
//! lumps' shells as `u32`s.
use super::topology::{BrepTopology, Sense};

pub(super) fn of(t: &BrepTopology) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    let counts = t.counts();
    for n in [
        counts.vertices,
        counts.edges,
        counts.coedges,
        counts.loops,
        counts.faces,
        counts.shells,
        counts.lumps,
    ] {
        h.update(&(n as u64).to_le_bytes());
    }
    let id = |h: &mut blake3::Hasher, i: Option<usize>| {
        h.update(&i.map_or(u32::MAX, |i| i as u32).to_le_bytes());
    };
    let precision = |h: &mut blake3::Hasher, p: Option<f64>| match p {
        Some(p) => {
            h.update(&[1]);
            h.update(&p.to_bits().to_le_bytes());
        }
        None => {
            h.update(&[0]);
        }
    };
    let sense = |h: &mut blake3::Hasher, s: Sense| {
        h.update(&[u8::from(s == Sense::Reversed)]);
    };
    for v in t.vertices.iter() {
        precision(&mut h, v.precision);
    }
    for e in t.edges.iter() {
        id(&mut h, e.curve.map(|c| c.index()));
        for x in e.interval {
            h.update(&x.to_bits().to_le_bytes());
        }
        for v in e.vertices {
            id(&mut h, Some(v.index()));
        }
        precision(&mut h, e.precision);
        id(&mut h, Some(e.coedge.index()));
    }
    for c in t.coedges.iter() {
        id(&mut h, Some(c.edge.index()));
        sense(&mut h, c.sense);
        id(&mut h, Some(c.owner.index()));
        id(&mut h, Some(c.radial.index()));
        id(&mut h, c.pcurve.map(|p| p.index()));
        precision(&mut h, c.precision);
    }
    for l in t.loops.iter() {
        id(&mut h, Some(l.face.index()));
        h.update(&l.first.to_le_bytes());
        h.update(&l.len.to_le_bytes());
    }
    for f in t.faces.iter() {
        id(&mut h, Some(f.surface.index()));
        sense(&mut h, f.sense);
        h.update(&f.first_loop.to_le_bytes());
        h.update(&f.loops.to_le_bytes());
        id(&mut h, Some(f.shell.index()));
        precision(&mut h, f.precision);
    }
    for s in t.shells.iter() {
        id(&mut h, Some(s.lump.index()));
        h.update(&s.first.to_le_bytes());
        h.update(&s.len.to_le_bytes());
        h.update(&[u8::from(s.closed)]);
    }
    for l in t.lumps.iter() {
        h.update(&l.first.to_le_bytes());
        h.update(&l.len.to_le_bytes());
    }
    for f in t.shell_faces.iter() {
        id(&mut h, Some(f.index()));
    }
    for s in t.lump_shells.iter() {
        id(&mut h, Some(s.index()));
    }
    *h.finalize().as_bytes()
}
