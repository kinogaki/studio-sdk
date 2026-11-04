//! Spatial searches over a value: the nearest place on its triangles or the
//! first hit along a ray, and the nearest points. A tree of boxes, built once
//! per cook and read from any thread.
use crate::geometry::Geometry;
use crate::vec3::Vec3;

/// How many items a leaf holds.
const LEAF: usize = 8;

/// A box.
#[derive(Clone, Copy, Debug)]
struct Cell {
    low: Vec3,
    high: Vec3,
}

impl Cell {
    const EMPTY: Cell = Cell {
        low: Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
        high: Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
    };

    fn with(self, p: Vec3) -> Self {
        Self {
            low: self.low.min(p),
            high: self.high.max(p),
        }
    }

    fn square_distance(&self, p: Vec3) -> f64 {
        let d = (self.low - p).max(p - self.high).max(Vec3::ZERO);
        d.dot(d)
    }

    /// Where a ray enters it, up to `far`.
    fn entered(&self, from: Vec3, inverse: Vec3, far: f64) -> Option<f64> {
        let (a, b) = (self.low - from, self.high - from);
        let t1 = Vec3::new(a.x * inverse.x, a.y * inverse.y, a.z * inverse.z);
        let t2 = Vec3::new(b.x * inverse.x, b.y * inverse.y, b.z * inverse.z);
        let (near, late) = (t1.min(t2), t1.max(t2));
        let start = near.x.max(near.y).max(near.z).max(0.0);
        let end = late.x.min(late.y).min(late.z).min(far);
        (start <= end).then_some(start)
    }
}

/// A node of a tree: its box, then two children or a run of items.
#[derive(Clone, Copy, Debug)]
struct Node {
    cell: Cell,
    /// `(start, end)` of `Tree::items` for a leaf; `right` otherwise.
    start: u32,
    end: u32,
    right: u32,
}

/// A tree over `items`' spots.
#[derive(Debug)]
struct Tree {
    nodes: Vec<Node>,
    items: Vec<u32>,
}

impl Tree {
    /// A tree of the items `0..count`, each spanning `cell(item)`.
    fn new(count: usize, cell: &dyn Fn(u32) -> Cell) -> Self {
        let mut items: Vec<u32> = (0..count as u32).collect();
        let middles: Vec<Vec3> = (0..count as u32)
            .map(|i| {
                let c = cell(i);
                (c.low + c.high) * 0.5
            })
            .collect();
        let mut nodes = Vec::new();
        Self::build(&mut nodes, &mut items, 0, count, cell, &middles);
        Self { nodes, items }
    }

    fn build(
        nodes: &mut Vec<Node>,
        items: &mut [u32],
        start: usize,
        end: usize,
        cell: &dyn Fn(u32) -> Cell,
        middles: &[Vec3],
    ) -> u32 {
        let mut spans = Cell::EMPTY;
        let mut mids = Cell::EMPTY;
        for &i in &items[start..end] {
            let c = cell(i);
            spans = spans.with(c.low).with(c.high);
            mids = mids.with(middles[i as usize]);
        }
        let at = nodes.len() as u32;
        nodes.push(Node {
            cell: spans,
            start: start as u32,
            end: end as u32,
            right: 0,
        });
        if end - start <= LEAF {
            return at;
        }
        let size = mids.high - mids.low;
        let key: fn(Vec3) -> f64 = if size.x >= size.y && size.x >= size.z {
            |v| v.x
        } else if size.y >= size.z {
            |v| v.y
        } else {
            |v| v.z
        };
        let half = (end - start) / 2;
        items[start..end].select_nth_unstable_by(half, |&a, &b| {
            key(middles[a as usize]).total_cmp(&key(middles[b as usize]))
        });
        let middle = start + half;
        Self::build(nodes, items, start, middle, cell, middles);
        let right = Self::build(nodes, items, middle, end, cell, middles);
        nodes[at as usize].right = right;
        at
    }
}

/// One triangle of a surface.
#[derive(Clone, Copy, Debug)]
struct Triangle {
    corners: [Vec3; 3],
    primitive: u32,
    /// The vertices of its corners.
    vertices: [u32; 3],
}

/// The nearest place on a surface, or a hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Where.
    pub at: Vec3,
    /// How far.
    pub distance: f64,
    /// The primitive there.
    pub primitive: u32,
    /// The vertices of the triangle there.
    pub vertices: [u32; 3],
    /// Their weights there.
    pub weights: [f64; 3],
    /// The triangle's unit normal.
    pub normal: Vec3,
}

/// A value's polygons as triangles.
#[derive(Debug)]
pub struct Surface {
    tree: Tree,
    triangles: Vec<Triangle>,
}

/// The nearest place on triangle `c` to `p`, and its weights (Ericson's).
fn on_triangle(c: &[Vec3; 3], p: Vec3) -> (Vec3, [f64; 3]) {
    let (ab, ac, ap) = (c[1] - c[0], c[2] - c[0], p - c[0]);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return (c[0], [1.0, 0.0, 0.0]);
    }
    let bp = p - c[1];
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return (c[1], [0.0, 1.0, 0.0]);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let t = d1 / (d1 - d3);
        return (c[0] + ab * t, [1.0 - t, t, 0.0]);
    }
    let cp = p - c[2];
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return (c[2], [0.0, 0.0, 1.0]);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let t = d2 / (d2 - d6);
        return (c[0] + ac * t, [1.0 - t, 0.0, t]);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let t = (d4 - d3) / (d4 - d3 + d5 - d6);
        return (c[1] + (c[2] - c[1]) * t, [0.0, 1.0 - t, t]);
    }
    let denom = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denom, vc * denom);
    (c[0] + ab * v + ac * w, [1.0 - v - w, v, w])
}

impl Surface {
    /// `value`'s polygons, each a fan of triangles.
    pub fn of(value: &Geometry) -> Self {
        let table = value.primitives();
        let at = |q: u32| Vec3::from(value.positions()[q as usize]);
        let mut triangles = Vec::new();
        for p in 0..table.len() {
            if !table.kind(p).is_polygon() {
                continue;
            }
            let row = table.primitive(p);
            let first = table.primitive_vertices(p).start as u32;
            for k in 1..row.len().saturating_sub(1) {
                triangles.push(Triangle {
                    corners: [at(row[0]), at(row[k]), at(row[k + 1])],
                    primitive: p as u32,
                    vertices: [first, first + k as u32, first + k as u32 + 1],
                });
            }
        }
        let cell = |i: u32| {
            let t = &triangles[i as usize];
            (t.corners.iter()).fold(Cell::EMPTY, |c, &p| c.with(p))
        };
        Self {
            tree: Tree::new(triangles.len(), &cell),
            triangles,
        }
    }

    fn hit(&self, t: &Triangle, at: Vec3, distance: f64, weights: [f64; 3]) -> Hit {
        let [a, b, c] = t.corners;
        let n = (b - a).cross(c - a);
        let length = n.length();
        Hit {
            at,
            distance,
            primitive: t.primitive,
            vertices: t.vertices,
            weights,
            normal: if length > 0.0 { n / length } else { Vec3::ZERO },
        }
    }

    /// The nearest place to `p`, within `reach`.
    pub fn nearest(&self, p: Vec3, reach: f64) -> Option<Hit> {
        if self.triangles.is_empty() {
            return None;
        }
        let mut best: Option<(f64, usize, Vec3, [f64; 3])> = None;
        let mut limit = reach * reach;
        let mut stack = vec![0u32];
        while let Some(n) = stack.pop() {
            let node = &self.tree.nodes[n as usize];
            if node.cell.square_distance(p) > limit {
                continue;
            }
            if node.right == 0 {
                for &i in &self.tree.items[node.start as usize..node.end as usize] {
                    let (at, weights) = on_triangle(&self.triangles[i as usize].corners, p);
                    let d = (at - p).dot(at - p);
                    if d <= limit {
                        limit = d;
                        best = Some((d, i as usize, at, weights));
                    }
                }
                continue;
            }
            let (left, right) = (n + 1, node.right);
            let d = |c: u32| self.tree.nodes[c as usize].cell.square_distance(p);
            if d(left) < d(right) {
                stack.extend([right, left]);
            } else {
                stack.extend([left, right]);
            }
        }
        let (d, i, at, weights) = best?;
        Some(self.hit(&self.triangles[i], at, d.sqrt(), weights))
    }

    /// The first hit along the ray from `from` along the unit `direction`,
    /// within `reach`.
    pub fn along(&self, from: Vec3, direction: Vec3, reach: f64) -> Option<Hit> {
        if self.triangles.is_empty() {
            return None;
        }
        let invert = |c: f64| if c != 0.0 { 1.0 / c } else { f64::INFINITY };
        let inverse = Vec3::new(
            invert(direction.x),
            invert(direction.y),
            invert(direction.z),
        );
        let mut far = reach;
        let mut best = None;
        let mut stack = vec![0u32];
        while let Some(n) = stack.pop() {
            let node = &self.tree.nodes[n as usize];
            if node.cell.entered(from, inverse, far).is_none() {
                continue;
            }
            if node.right != 0 {
                stack.extend([node.right, n + 1]);
                continue;
            }
            for &i in &self.tree.items[node.start as usize..node.end as usize] {
                let t = &self.triangles[i as usize];
                let [a, b, c] = t.corners;
                let (ab, ac) = (b - a, c - a);
                let h = direction.cross(ac);
                let det = ab.dot(h);
                if det.abs() < 1e-14 {
                    continue;
                }
                let s = from - a;
                let u = s.dot(h) / det;
                let q = s.cross(ab);
                let v = direction.dot(q) / det;
                let along = ac.dot(q) / det;
                if u >= 0.0 && v >= 0.0 && u + v <= 1.0 && along >= 0.0 && along <= far {
                    far = along;
                    best = Some(self.hit(t, from + direction * along, along, [1.0 - u - v, u, v]));
                }
            }
        }
        best
    }
}

/// A set of spots: which are nearest a place.
#[derive(Debug)]
pub struct Spots {
    tree: Tree,
    spots: Vec<Vec3>,
}

impl Spots {
    /// A search over `spots`, the items numbered in their order.
    pub fn of(spots: Vec<Vec3>) -> Self {
        let cell = |i: u32| {
            let p = spots[i as usize];
            Cell { low: p, high: p }
        };
        Self {
            tree: Tree::new(spots.len(), &cell),
            spots,
        }
    }

    /// Up to `most` items within `reach` of `p`, nearest first, as `(item,
    /// distance)`. When none lies within, the nearest of all with `always`.
    pub fn nearest(&self, p: Vec3, most: usize, reach: f64, always: bool) -> Vec<(u32, f64)> {
        let mut found = self.within(p, most, reach * reach);
        if found.is_empty() && always {
            found = self.within(p, 1, f64::INFINITY);
        }
        found.into_iter().map(|(i, d)| (i, d.sqrt())).collect()
    }

    fn within(&self, p: Vec3, most: usize, square: f64) -> Vec<(u32, f64)> {
        let mut found: Vec<(u32, f64)> = Vec::new();
        if self.spots.is_empty() || most == 0 {
            return found;
        }
        let mut stack = vec![0u32];
        while let Some(n) = stack.pop() {
            let node = &self.tree.nodes[n as usize];
            let limit = if found.len() == most {
                found[most - 1].1
            } else {
                square
            };
            if node.cell.square_distance(p) > limit {
                continue;
            }
            if node.right != 0 {
                stack.extend([node.right, n + 1]);
                continue;
            }
            for &i in &self.tree.items[node.start as usize..node.end as usize] {
                let d = self.spots[i as usize] - p;
                let d = d.dot(d);
                let limit = if found.len() == most {
                    found[most - 1].1
                } else {
                    square
                };
                if d <= limit {
                    let at = found.partition_point(|f| (f.1, f.0) < (d, i));
                    found.insert(at, (i, d));
                    found.truncate(most);
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attributes, Primitives};

    /// A cube of side 1 about the origin.
    fn cube() -> Geometry {
        let s = 0.5;
        let positions: Vec<[f64; 3]> = vec![
            [-s, -s, -s],
            [s, -s, -s],
            [s, -s, s],
            [-s, -s, s],
            [-s, s, -s],
            [s, s, -s],
            [s, s, s],
            [-s, s, s],
        ];
        let corners = vec![
            0, 1, 2, 3, 4, 7, 6, 5, 0, 4, 5, 1, 2, 6, 7, 3, 3, 7, 4, 0, 1, 5, 6, 2,
        ];
        let offsets: Vec<u32> = (0..=6).map(|f| 4 * f).collect();
        let table = Primitives::of_polygons(8, offsets.into(), corners.into()).unwrap();
        Geometry::new(positions.into(), table, Attributes::new()).unwrap()
    }

    /// A unit plane of `n` × `n` quads about the origin.
    fn plane(n: u32) -> Geometry {
        let step = 1.0 / f64::from(n);
        let positions: Vec<[f64; 3]> = (0..=n)
            .flat_map(|j| {
                (0..=n).map(move |i| [f64::from(i) * step - 0.5, 0.0, f64::from(j) * step - 0.5])
            })
            .collect();
        let at = |i: u32, j: u32| j * (n + 1) + i;
        let corners: Vec<u32> = (0..n)
            .flat_map(|j| {
                (0..n).flat_map(move |i| [at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j)])
            })
            .collect();
        let offsets: Vec<u32> = (0..=n * n).map(|f| 4 * f).collect();
        let points = positions.len();
        let table = Primitives::of_polygons(points, offsets.into(), corners.into()).unwrap();
        Geometry::new(positions.into(), table, Attributes::new()).unwrap()
    }

    #[test]
    fn the_nearest_place_and_a_ray_find_a_cubes_face() {
        let surface = Surface::of(&cube());
        let hit = surface
            .nearest(Vec3::new(0.1, 0.0, 3.0), f64::INFINITY)
            .unwrap();
        assert!((hit.at - Vec3::new(0.1, 0.0, 0.5)).length() < 1e-12);
        assert!((hit.distance - 2.5).abs() < 1e-12);
        assert!((hit.normal - Vec3::Z).length() < 1e-12);
        let sum: f64 = hit.weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12);
        let ray = surface
            .along(Vec3::new(0.1, 0.2, 3.0), Vec3::Z * -1.0, 100.0)
            .unwrap();
        assert!((ray.at - Vec3::new(0.1, 0.2, 0.5)).length() < 1e-12);
        assert!(
            surface
                .along(Vec3::new(0.1, 0.2, 3.0), Vec3::Z, 100.0)
                .is_none()
        );
        assert!(surface.nearest(Vec3::new(0.0, 0.0, 3.0), 1.0).is_none());
    }

    #[test]
    fn a_search_over_spots_agrees_with_trying_them_all() {
        let value = plane(16);
        let spots: Vec<Vec3> = value.positions().iter().map(|&p| p.into()).collect();
        let search = Spots::of(spots.clone());
        for i in 0..50 {
            let p = Vec3::new(f64::from(i) * 0.031 - 0.7, 0.1, 0.6 - f64::from(i) * 0.02);
            let mut all: Vec<(u32, f64)> = (spots.iter().enumerate())
                .map(|(k, &s)| (k as u32, (s - p).length()))
                .collect();
            all.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            let found = search.nearest(p, 5, f64::INFINITY, false);
            for (a, b) in found.iter().zip(&all) {
                assert!((a.1 - b.1).abs() < 1e-12);
            }
            assert_eq!(found.len(), 5);
        }
        assert!(
            search
                .nearest(Vec3::new(9.0, 0.0, 0.0), 5, 1.0, false)
                .is_empty()
        );
        assert_eq!(
            search.nearest(Vec3::new(9.0, 0.0, 0.0), 5, 1.0, true).len(),
            1
        );
    }
}
