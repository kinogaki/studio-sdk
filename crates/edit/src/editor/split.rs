//! The editor's splits, each a few of its plain changes: an edge, a
//! polygon, a point.
use super::{Editor, one};
use std::collections::HashMap;

impl Editor {
    /// A new point on the edge between `a` and `b`, `t` of the way from
    /// `a`; its number. Every primitive with `a` and `b` as neighbouring
    /// corners gets a corner there, its Vertex attributes blended from
    /// those two; the point's come from `a` and `b` and both halves of the
    /// edge keep the edge's values. An edge nothing uses still gets the
    /// point.
    pub fn split_edge(&mut self, a: u32, b: u32, t: f64) -> u32 {
        if !self.check_point(a) || !self.check_point(b) {
            return self.point_count();
        }
        let (from, to) = (self.position(a), self.position(b));
        let at = std::array::from_fn(|i| from[i] + (to[i] - from[i]) * t);
        let q = self.add_point(at, &[(a, 1.0 - t), (b, t)]);
        for p in self.primitives_on(a, b) {
            let mut corners = self.corners_with_sources(p);
            let closed = self.kind(p).closed();
            let n = corners.len();
            let gap = (0..n).find(|&i| {
                let next = i + 1 < n || closed;
                let (x, y) = (corners[i].0, corners[(i + 1) % n].0);
                next && ((x == a && y == b) || (x == b && y == a))
            });
            let Some(i) = gap else { continue };
            let (x, y) = (corners[i], corners[(i + 1) % n]);
            let toward_y = if x.0 == a { t } else { 1.0 - t };
            let source = self.vertex_blends.mixed(x.1, y.1, toward_y);
            corners.insert(i + 1, (q, source));
            let kind = self.kind(p);
            let row = self.write_spans(kind, p, &corners);
            self.set_row(p, row);
        }
        self.carry_edge([a, q], [a, b]);
        self.carry_edge([q, b], [a, b]);
        q
    }

    /// Polygon `p` cut between its corners at positions `first` and
    /// `second` (not neighbours): it keeps the corners from `first` to
    /// `second`, and a new polygon with its attributes takes the corners
    /// from `second` round to `first`; the new polygon's number. The cut
    /// is a new edge, without values to carry.
    pub fn split_polygon(&mut self, p: u32, first: usize, second: usize) -> u32 {
        if !self.check_primitive(p) {
            return self.primitive_count();
        }
        let corners = self.corners_with_sources(p);
        let n = corners.len();
        let (low, high) = (first.min(second), first.max(second));
        let neighbours = high - low < 2 || (low == 0 && high == n - 1);
        if !self.kind(p).is_polygon() || high >= n || neighbours {
            self.refuse(format!(
                "primitive {p} can't be cut at {first} and {second}"
            ));
            return self.primitive_count();
        }
        let kind = self.kind(p);
        let kept = self.write_spans(kind, p, &corners[low..=high]);
        self.set_row(p, kept);
        let number = self.primitive_count();
        let rest: Vec<_> = corners[high..]
            .iter()
            .chain(&corners[..=low])
            .copied()
            .collect();
        let base = self.base_primitives();
        let span = (self.primitive_blends)
            .add(&one(p), base, &self.primitive_from)
            .expect("a known primitive at full weight");
        let row = self.write_spans(kind, number, &rest);
        self.added_rows.push(row);
        self.primitive_from.push(span);
        number
    }

    /// A new point copying `p`, which the primitives `of` name instead of
    /// it; its number. What pulling a seam apart is.
    pub fn split_point(&mut self, p: u32, of: &[u32]) -> u32 {
        if !self.check_point(p) {
            return self.point_count();
        }
        let q = self.add_point(self.position(p), &one(p));
        for &f in of {
            if !self.check_primitive(f) {
                continue;
            }
            let mut corners = self.corners_with_sources(f);
            corners
                .iter_mut()
                .filter(|c| c.0 == p)
                .for_each(|c| c.0 = q);
            let kind = self.kind(f);
            let row = self.write_spans(kind, f, &corners);
            self.set_row(f, row);
        }
        q
    }

    /// The primitives with `a` and `b` on one row, as the edit has them:
    /// the base's on that edge and rows it edited. Costs the index once
    /// per table, then what it reads. A row may still not hold them as
    /// neighbours; the caller looks.
    fn primitives_on(&mut self, a: u32, b: u32) -> Vec<u32> {
        let table = self.base.primitives();
        let base = table.len() as u32;
        let mut on = Vec::new();
        let in_base = a < self.base_points() && b < self.base_points();
        if let Some(edge) = in_base.then(|| table.edge_of(a, b)).flatten() {
            on.extend(table.index().edge_primitives(edge as usize));
        }
        let through = self.edited_through.get_or_insert_with(|| {
            let mut through: HashMap<u32, Vec<u32>> = HashMap::new();
            let rows = (self.rewired.iter().map(|(&p, row)| (p, row)))
                .chain((self.added_rows.iter().enumerate()).map(|(k, row)| (base + k as u32, row)));
            for (p, row) in rows {
                for &point in &self.corners[row.start as usize..(row.start + row.len) as usize] {
                    through.entry(point).or_default().push(p);
                }
            }
            through
        });
        if let (Some(through_a), Some(through_b)) = (through.get(&a), through.get(&b)) {
            on.extend(through_a.iter().filter(|p| through_b.contains(p)));
        }
        on.sort_unstable();
        on.dedup();
        on.retain(|p| !self.removed_primitives.contains(p));
        on
    }
}
