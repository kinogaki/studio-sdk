//! Seeded edits against a model.
use super::fixture::*;
use kinogaki_edit::editor::{Corner, Editor};
use kinogaki_geometry::{Domain, Kind, names};

/// A primitive's id (none when the edit added it) and its corners: each
/// one's point and the base vertex it came from (-1: none).
type ModelRow = (Option<i64>, Vec<(u32, i32)>);

/// A small deterministic generator.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u32) -> u32 {
        self.0 = (self.0)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0xD1B5_4A32_D192_ED03);
        ((self.0 >> 33) as u32) % n.max(1)
    }
}

/// Seeded runs of removals, rewires and additions against a model holding
/// each primitive's id and its corners' points: by ids, the result is the
/// model, and its Vertex values went with their rows.
#[test]
fn seeded_edits_agree_with_a_model_kept_by_ids() {
    for seed in 0..64 {
        let mut rng = Rng(seed);
        let value = ruled(4, 3);
        let table = value.primitives();
        let point_ids = column::<i64>(&value, Domain::Point, names::ID);
        let prim_ids = column::<i64>(&value, Domain::Primitive, names::ID);
        let mut model: Vec<ModelRow> = (0..table.len())
            .map(|p| {
                let corners = table.primitive(p).iter().copied();
                let vertices = table.primitive_vertices(p).map(|v| v as i32);
                (Some(prim_ids[p]), corners.zip(vertices).collect())
            })
            .collect();
        let mut alive: Vec<bool> = vec![true; table.points()];
        let mut edit = Editor::new(&value);
        for _ in 0..1 + rng.below(12) {
            match rng.below(4) {
                0 => {
                    let p = rng.below(model.len() as u32);
                    if !model[p as usize].1.is_empty() {
                        edit.remove_primitive(p);
                        model[p as usize].1.clear();
                    }
                }
                1 => {
                    let q = rng.below(alive.len() as u32);
                    if alive[q as usize] {
                        edit.remove_point(q);
                        alive[q as usize] = false;
                        for row in &mut model {
                            row.1.retain(|c| c.0 != q);
                            if row.1.len() < 3 {
                                row.1.clear();
                            }
                        }
                    }
                }
                2 => {
                    // A row turned the other way, each corner's vertex with it.
                    let p = rng.below(model.len() as u32);
                    let row = &mut model[p as usize].1;
                    if row.is_empty() || row[0].1 < 0 {
                        continue;
                    }
                    row.reverse();
                    let sources: Vec<[(u32, f64); 1]> =
                        row.iter().map(|c| [(c.1 as u32, 1.0)]).collect();
                    let new: Vec<Corner<'_>> = (row.iter().zip(&sources))
                        .map(|(c, from)| Corner { point: c.0, from })
                        .collect();
                    edit.rewire(p, &new);
                }
                _ => {
                    let at = [f64::from(rng.below(9)); 3];
                    let q = edit.add_point(at, &[]);
                    alive.push(true);
                    let others: Vec<u32> = (0..q).filter(|&c| alive[c as usize]).collect();
                    let n = others.len() as u32;
                    if n >= 2 {
                        let (a, b) = (others[rng.below(n) as usize], others[rng.below(n) as usize]);
                        if a != b {
                            edit.add_primitive(Kind::POLYGON, &corners(&[q, a, b]), &[]);
                            model.push((None, vec![(q, -1), (a, -1), (b, -1)]));
                        }
                    }
                }
            }
        }
        let (out, change) = edit.finish().unwrap();
        let kept: Vec<_> = model.iter().filter(|r| !r.1.is_empty()).collect();
        let t = out.primitives();
        assert_eq!(t.len(), kept.len(), "seed {seed}");
        let new_ids = column::<i64>(&out, Domain::Primitive, names::ID);
        let new_points = column::<i64>(&out, Domain::Point, names::ID);
        let corner = column::<i32>(&out, Domain::Vertex, "corner");
        for (p, (id, row)) in kept.iter().enumerate() {
            if let Some(id) = id {
                assert_eq!(new_ids[p], *id, "seed {seed}");
            }
            for (k, &(point, vertex)) in row.iter().enumerate() {
                let now = t.primitive(p)[k];
                assert_eq!(Some(now), change.points.of(point), "seed {seed}");
                if (point as usize) < point_ids.len() {
                    let id = point_ids[point as usize];
                    assert_eq!(new_points[now as usize], id, "seed {seed}");
                }
                let v = t.primitive_vertices(p).start + k;
                let want = vertex.max(0);
                assert_eq!(corner[v], want, "seed {seed}, primitive {p}, corner {k}");
            }
        }
        let alive = alive.iter().filter(|&&a| a).count();
        assert_eq!(t.points(), alive, "seed {seed}");
    }
}
