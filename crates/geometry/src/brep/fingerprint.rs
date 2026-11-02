//! The fingerprints a selection keeps: whether component indices still
//! mean the same elements. They hash structure alone, never parameter
//! ranges, precisions, carriers or any geometry, so a resize (a box's
//! width, a fillet's radius over the same topology) keeps them and a
//! change of structure (a face added or split) changes them.
//!
//! - **Topology** ([`of`]): BLAKE3 of the little-endian stream of the
//!   counts (vertices, edges, coedges, loops, faces, shells, lumps, as
//!   `u64`), then the adjacency, table by table in that order, field by
//!   field as declared: each edge's vertices and first coedge; each
//!   coedge's edge, sense (one byte, `Same` 0), loop and radial; each
//!   loop's face, first coedge and length; each face's first loop, loop
//!   count and shell; each shell's lump, first face, length and whether
//!   it is closed (one byte); each lump's first shell and length; then the
//!   shells' faces and the lumps' shells. Ids are `u32`.
//! - **Names** ([`names`]): BLAKE3 of the faces', edges' and vertices'
//!   persistent names in that order, each table's length as a `u64` first,
//!   each name as its node's tag byte and fields (texts as their `u64`
//!   byte length and bytes, names within names in place).
use super::names::{End, Name, NameNode, Role};
use super::topology::{BrepTopology, Names, Sense};

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
    let id = |h: &mut blake3::Hasher, i: usize| {
        h.update(&(i as u32).to_le_bytes());
    };
    for e in t.edges.iter() {
        for v in e.vertices {
            id(&mut h, v.index());
        }
        id(&mut h, e.coedge.index());
    }
    for c in t.coedges.iter() {
        id(&mut h, c.edge.index());
        h.update(&[u8::from(c.sense == Sense::Reversed)]);
        id(&mut h, c.owner.index());
        id(&mut h, c.radial.index());
    }
    for l in t.loops.iter() {
        id(&mut h, l.face.index());
        h.update(&l.first.to_le_bytes());
        h.update(&l.len.to_le_bytes());
    }
    for f in t.faces.iter() {
        h.update(&f.first_loop.to_le_bytes());
        h.update(&f.loops.to_le_bytes());
        id(&mut h, f.shell.index());
    }
    for s in t.shells.iter() {
        id(&mut h, s.lump.index());
        h.update(&s.first.to_le_bytes());
        h.update(&s.len.to_le_bytes());
        h.update(&[u8::from(s.closed)]);
    }
    for l in t.lumps.iter() {
        h.update(&l.first.to_le_bytes());
        h.update(&l.len.to_le_bytes());
    }
    for f in t.shell_faces.iter() {
        id(&mut h, f.index());
    }
    for s in t.lump_shells.iter() {
        id(&mut h, s.index());
    }
    *h.finalize().as_bytes()
}

pub(super) fn names(n: &Names) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    for table in [&n.faces, &n.edges, &n.vertices] {
        h.update(&(table.len() as u64).to_le_bytes());
        for name in table.iter() {
            hash_name(&mut h, name);
        }
    }
    *h.finalize().as_bytes()
}

fn int(h: &mut blake3::Hasher, i: u32) {
    h.update(&i.to_le_bytes());
}

fn text(h: &mut blake3::Hasher, t: &str) {
    h.update(&(t.len() as u64).to_le_bytes());
    h.update(t.as_bytes());
}

fn hash_name(h: &mut blake3::Hasher, name: &Name) {
    match name.node() {
        NameNode::Created { by, role } => {
            h.update(&[0]);
            h.update(&by.0.to_le_bytes());
            hash_role(h, role);
        }
        NameNode::Part { of, index } => {
            h.update(&[1]);
            hash_name(h, of);
            int(h, index);
        }
        NameNode::Merged { of } => {
            h.update(&[2]);
            h.update(&(of.len() as u64).to_le_bytes());
            for n in of {
                hash_name(h, n);
            }
        }
    }
}

fn hash_role(h: &mut blake3::Hasher, role: &Role) {
    match role {
        Role::Local { index } => {
            h.update(&[0]);
            int(h, *index);
        }
        Role::Primitive { index } => {
            h.update(&[1]);
            int(h, *index);
        }
        Role::Imported { body, index } => {
            h.update(&[2]);
            int(h, *body);
            int(h, *index);
        }
        Role::ImportedBody { body } => {
            h.update(&[3]);
            int(h, *body);
        }
        Role::Cap { profile, end } => {
            h.update(&[4]);
            text(h, profile);
            h.update(&[u8::from(*end == End::End)]);
        }
        Role::Side {
            profile,
            curve,
            segment,
        } => {
            h.update(&[5]);
            text(h, profile);
            int(h, *curve);
            int(h, *segment);
        }
        Role::Along(n) => {
            h.update(&[6]);
            hash_name(h, n);
        }
        Role::At(n) => {
            h.update(&[7]);
            hash_name(h, n);
        }
        Role::Offset(n) => {
            h.update(&[8]);
            hash_name(h, n);
        }
        Role::Copy { of, index } => {
            h.update(&[9]);
            hash_name(h, of);
            int(h, *index);
        }
        Role::Between { faces, index } => {
            h.update(&[10]);
            h.update(&(faces.len() as u64).to_le_bytes());
            for n in faces.iter() {
                hash_name(h, n);
            }
            int(h, *index);
        }
    }
}
