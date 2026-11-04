//! [`Editor::finish`]: the changes made into a new value, in one pass over
//! what they reach; everything else stays shared. `settle` says what they
//! come to.
use super::Editor;
use super::carry::{carried_attribute, carried_group};
use super::change::{Change, Numbers};
use super::layout::{Blends, Layout, Run, Span};
use super::settle::{Filling, GONE, Points, Settled, Vertex};
use kinogaki_geometry::{
    Attributes, Basis, Column, ColumnWriter, CurveData, Domain, Error, Geometry, Knots, Primitives,
    TableWriter,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// `base` with `points` at `at` and nothing else changed: an edit's finish when
/// it moved only points, with no list of changes on the way, so a deformer that
/// moves every point of a million in each cook of a drag costs one write.
///
/// ```
/// use kinogaki_edit::editor::place;
/// use kinogaki_geometry::{Attributes, Geometry, Primitives};
/// use std::sync::Arc;
///
/// let t = Primitives::of_polygons(3, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
/// let flat = Geometry::new(vec![[0.0; 3]; 3].into(), t, Attributes::new()).unwrap();
/// let (lifted, change) = place(&flat, &[2], &[[0.0, 0.0, 1.0]]).unwrap();
/// assert!(Arc::ptr_eq(lifted.primitives(), flat.primitives()));
/// assert_eq!(change.moved, [2]);
/// ```
///
/// # Errors
/// A point past the last, a bad position, or not one position a point.
pub fn place(
    base: &Geometry,
    points: &[u32],
    at: &[[f64; 3]],
) -> Result<(Geometry, Change), Error> {
    if points.is_empty() {
        return Ok((base.clone(), Change::default()));
    }
    let value = base.with_moved(points, at)?;
    let moved = value.moved_points(base);
    let change = Change {
        moved,
        ..Change::default()
    };
    Ok((value, change))
}

/// Where a new table's elements came from.
struct Rows {
    primitives: Layout,
    vertices_from: Layout,
    rewired: Vec<u32>,
    removed: Vec<u32>,
    filled: Vec<(u32, u32)>,
}

impl Editor {
    /// The new value and what changed. Nothing changed: the value itself.
    ///
    /// # Errors
    /// The first bad number or source, or a rule the result breaks (a
    /// polygon repeating a point after a weld, a bad position).
    pub fn finish(mut self) -> Result<(Geometry, Change), Error> {
        if let Some(fault) = self.fault.take() {
            return Err(fault);
        }
        let structural = !self.rewired.is_empty()
            || !self.added_rows.is_empty()
            || !self.removed_primitives.is_empty()
            || !self.removed_points.is_empty()
            || !self.welded.is_empty()
            || self.loose_go;
        if !structural && self.added_points.is_empty() {
            return self.moved_only();
        }
        self.remade()
    }

    /// Points moved and nothing else: the cells of the positions they fall in.
    fn moved_only(self) -> Result<(Geometry, Change), Error> {
        let mut moved: Vec<(u32, [f64; 3])> = self.moved.into_iter().collect();
        moved.sort_unstable_by_key(|m| m.0);
        let (points, at): (Vec<u32>, Vec<[f64; 3]>) = moved.into_iter().unzip();
        place(&self.base, &points, &at)
    }

    /// Anything else: what the changes reach is written again.
    fn remade(self) -> Result<(Geometry, Change), Error> {
        let settled = self.settle();
        let points = self.points(&settled);
        let table = self.base.primitives();
        let same_rows = settled.rows.is_empty() && settled.gone.is_empty() && !points.renumbered();
        let (primitives, rows) = if !same_rows {
            let (writer, rows) = self.rows(&points, &settled)?;
            let curves = self.curves_of(&rows);
            (Arc::new(writer.finish(curves)?), Some(rows))
        } else if points.layout.len() == table.points() {
            (table.clone(), None)
        } else {
            (Arc::new(table.with_points(points.layout.len())?), None)
        };
        let positions = self.positions(&points);
        let attributes = self.attributes(&points, rows.as_ref(), &primitives)?;
        let topology = !Arc::ptr_eq(&primitives, table);
        let change = self.change(&points, rows, topology);
        let value = self.base.edited(positions, primitives, attributes)?;
        Ok((value, change))
    }

    /// The table's rows as the edit has them: those it didn't reach keep
    /// their numbers and share their cells; the rest are written once each.
    fn rows<'a>(
        &'a self,
        points: &Points,
        settled: &Settled,
    ) -> Result<(TableWriter<'a>, Rows), Error> {
        let table = self.base.primitives();
        let base = table.len() as u32;
        let wants_vertices = self.base.attributes().has(Domain::Vertex)
            || (table.curves()).is_some_and(|c| c.weights().is_some());
        let of_base = settled.gone.partition_point(|&g| g < base);
        let filling = Filling::of(base, &settled.gone[..of_base]);
        // The rows naming a point that took another number.
        let mut renamed: Vec<u32> = Vec::new();
        for &(p, _) in &points.filling.filled {
            renamed.extend(table.index().point_primitives(p as usize));
        }
        renamed.retain(|r| {
            !settled.rows.contains_key(r) && settled.gone[..of_base].binary_search(r).is_err()
        });
        renamed.sort_unstable();
        renamed.dedup();
        // The rows to write, by their new numbers, and what each is now.
        let kept = |r: &u32| *r < filling.kept;
        let mut written: Vec<(u32, u32)> = (settled.rows.keys().copied())
            .chain(renamed.iter().copied())
            .filter(kept)
            .map(|r| (r, r))
            .collect();
        written.extend(filling.filled.iter().map(|&(row, hole)| (hole, row)));
        written.sort_unstable();
        let mut writer = table.rewrite(points.layout.len());
        let mut rows = Rows {
            primitives: Layout::default(),
            vertices_from: Layout::default(),
            rewired: Vec::new(),
            removed: settled.gone.clone(),
            filled: Vec::new(),
        };
        let mut next = 0;
        for &(number, row) in &written {
            self.keep_rows(&mut writer, &mut rows, next..number, wants_vertices);
            next = number + 1;
            self.table_row(&mut writer, &mut rows, row, points, settled, wants_vertices)?;
            rows.primitives.keep(row..row + 1);
            if settled.rows.contains_key(&row) || renamed.binary_search(&row).is_ok() {
                rows.rewired.push(number);
            }
        }
        self.keep_rows(&mut writer, &mut rows, next..filling.kept, wants_vertices);
        for (k, &from) in self.primitive_from.iter().enumerate() {
            let number = base + k as u32;
            if settled.rows.contains_key(&number) {
                self.table_row(
                    &mut writer,
                    &mut rows,
                    number,
                    points,
                    settled,
                    wants_vertices,
                )?;
                rows.primitives.add(from);
            }
        }
        rows.filled = filling.filled;
        Ok((writer, rows))
    }

    /// `run`'s base rows, as they are.
    fn keep_rows(
        &self,
        writer: &mut TableWriter<'_>,
        rows: &mut Rows,
        run: std::ops::Range<u32>,
        wants_vertices: bool,
    ) {
        if run.is_empty() {
            return;
        }
        let offsets = self.base.primitives().offsets();
        let (first, last) = (offsets[run.start as usize], offsets[run.end as usize]);
        writer.keep(run.start as usize..run.end as usize);
        rows.primitives.keep(run);
        if wants_vertices {
            rows.vertices_from.keep(first..last);
        }
    }

    /// Row `row` of the edit, as it ended up or as the base has it, under the
    /// points' new numbers.
    fn table_row(
        &self,
        writer: &mut TableWriter<'_>,
        rows: &mut Rows,
        row: u32,
        points: &Points,
        settled: &Settled,
        wants_vertices: bool,
    ) -> Result<(), Error> {
        if let Some(found) = settled.rows.get(&row) {
            let corners = &settled.corners[found.corners.clone()];
            let numbers: Vec<u32> = corners.iter().map(|c| points.number(c.0)).collect();
            writer.write(found.kind, &numbers)?;
            for &(_, vertex) in corners.iter().filter(|_| wants_vertices) {
                match vertex {
                    Vertex::Kept(v) => rows.vertices_from.keep(v..v + 1),
                    Vertex::Blended(span) => rows.vertices_from.add(span),
                }
            }
            return Ok(());
        }
        let table = self.base.primitives();
        let numbers: Vec<u32> = (table.primitive(row as usize).iter())
            .map(|&p| points.number(p))
            .collect();
        writer.write(table.kind(row as usize), &numbers)?;
        if wants_vertices {
            let range = table.primitive_vertices(row as usize);
            rows.vertices_from
                .keep(range.start as u32..range.end as u32);
        }
        Ok(())
    }

    /// The curve data of `rows`: degrees and knots by primitive, weights by
    /// vertex; present while a curve is. A row the edit wrote weighs 1.
    fn curves_of(&self, rows: &Rows) -> Option<CurveData> {
        let table = self.base.primitives();
        let data = table.curves()?;
        let (mut degrees, mut knot_offsets, mut knots) = (Vec::new(), vec![0u32], Vec::new());
        let mut curved = false;
        for run in rows.primitives.runs() {
            match run {
                Run::Kept(range) => {
                    for p in range.clone() {
                        degrees.push(data.degrees()[p as usize]);
                        knots.extend_from_slice(data.knots().of(p as usize));
                        knot_offsets.push(knots.len() as u32);
                        curved |= table.kind(p as usize).basis() != Basis::Linear;
                    }
                }
                Run::New(_) => {
                    degrees.push(1);
                    knot_offsets.push(knots.len() as u32);
                }
            }
        }
        if !curved {
            return None;
        }
        let weights = data.weights().map(|w| {
            let mut out = Vec::with_capacity(rows.vertices_from.len());
            for run in rows.vertices_from.runs() {
                match run {
                    Run::Kept(range) => out
                        .extend_from_slice(&w.as_slice()[range.start as usize..range.end as usize]),
                    Run::New(_) => out.push(1.0),
                }
            }
            Column::new(out)
        });
        let knots = Knots::new(knot_offsets.into(), knots.into());
        Some(CurveData::new(degrees.into(), knots, weights))
    }

    /// The positions: kept ones as moved, new ones as placed.
    fn positions(&self, points: &Points) -> Column<[f64; 3]> {
        let base = self.base.positions();
        let layout = &points.layout;
        if self.moved.is_empty() && layout.is_whole(base.len()) {
            return base.clone();
        }
        let mut out = ColumnWriter::new();
        let mut added = points.positions.iter();
        for run in layout.runs() {
            match run {
                Run::Kept(range) => out.keep(base, range.start as usize..range.end as usize),
                Run::New(_) => out.push(*added.next().expect("a position a new point")),
            }
        }
        for (&p, &at) in &self.moved {
            let number = points.number(p);
            if number != GONE {
                out[number as usize] = at;
            }
        }
        out.finish()
    }

    /// The attributes and groups as the layouts say, sharing every domain
    /// that stays.
    fn attributes(
        &self,
        points: &Points,
        rows: Option<&Rows>,
        primitives: &Primitives,
    ) -> Result<Arc<Attributes>, Error> {
        let table = self.base.primitives();
        let base = self.base.attributes();
        let mut out = (**base).clone();
        let mut changed = false;
        let mut carry = |domain: Domain, layout: &Layout, blends: &Blends| -> Result<(), Error> {
            for (d, name, attribute) in base.iter().filter(|a| a.0 == domain) {
                out = out.with(d, name, carried_attribute(name, attribute, layout, blends)?)?;
                changed = true;
            }
            for (d, name, group) in base.groups().filter(|g| g.0 == domain) {
                out = out.with_group(d, name, carried_group(group, layout, blends))?;
                changed = true;
            }
            Ok(())
        };
        if !points.layout.is_whole(table.points()) {
            carry(Domain::Point, &points.layout, &self.point_blends)?;
        }
        if let Some(rows) = rows {
            if !rows.primitives.is_whole(table.len()) {
                carry(Domain::Primitive, &rows.primitives, &self.primitive_blends)?;
            }
            if base.has(Domain::Vertex) && !rows.vertices_from.is_whole(table.vertex_count()) {
                carry(Domain::Vertex, &rows.vertices_from, &self.vertex_blends)?;
            }
            if base.has(Domain::Edge) {
                let edges = self.edges_layout(primitives, points);
                if !edges.is_whole(table.edge_count()) {
                    carry(Domain::Edge, &edges, &Blends::default())?;
                }
            }
        }
        Ok(if changed { Arc::new(out) } else { base.clone() })
    }

    /// The new table's edges by the base's they take their values from: an
    /// edge between points that kept their numbers, one that followed its
    /// points to new numbers, or one [`carry_edge`](Editor::carry_edge)
    /// names.
    fn edges_layout(&self, new: &Primitives, points: &Points) -> Layout {
        let table = self.base.primitives();
        let index = table.index();
        let number = |p: u32| points.number(self.survivor(p));
        let pair = |a: u32, b: u32| [a.min(b), a.max(b)];
        // A declaration stands over an edge that stays too.
        let mut declared: HashMap<[u32; 2], u32> = HashMap::new();
        for (to, [a, b]) in &self.edge_from {
            let in_base = *a < self.base_points() && *b < self.base_points();
            let found = in_base.then(|| table.edge_of(*a, *b)).flatten();
            let [x, y] = to.map(number);
            if let Some(edge) = found.filter(|_| x != GONE && y != GONE && x != y) {
                declared.insert(pair(x, y), edge);
            }
        }
        // The base's edges on points gone or moved, by what they join now.
        let mut followed: HashMap<[u32; 2], u32> = HashMap::new();
        let filled = &points.filling.filled;
        let of_base = (points.gone.iter().copied()).filter(|&g| g < self.base_points());
        for p in of_base.chain(filled.iter().map(|f| f.0)) {
            for e in index.point_every_edge(p as usize) {
                let [x, y] = table.edge(e as usize).points.map(number);
                if x != GONE && y != GONE && x != y {
                    followed.entry(pair(x, y)).or_insert(e);
                }
            }
        }
        let holes: HashSet<u32> = filled.iter().map(|f| f.1).collect();
        let same = |p: u32| p < points.filling.kept && !holes.contains(&p);
        let from = |[a, b]: [u32; 2]| {
            let stays = || (same(a) && same(b)).then(|| table.edge_of(a, b)).flatten();
            (declared.get(&[a, b]).copied())
                .or_else(stays)
                .or_else(|| followed.get(&[a, b]).copied())
        };
        let mut layout = Layout::default();
        let mut next = |edge: Option<u32>| match edge {
            Some(e) => layout.keep(e..e + 1),
            None => layout.add(Span::default()),
        };
        if declared.is_empty() {
            new.index().compare_edges(index, |edge| match edge {
                Ok(kept) => kept.for_each(|e| next(Some(e))),
                Err(edge) => next(from(edge.points)),
            });
        } else {
            new.edges().for_each(|edge| next(from(edge.points)));
        }
        layout
    }

    /// The report.
    fn change(&self, points: &Points, rows: Option<Rows>, topology: bool) -> Change {
        let base = self.base.positions();
        let mut moved: Vec<u32> = (self.moved.iter())
            .filter(|(p, at)| base[**p as usize].map(f64::to_bits) != at.map(f64::to_bits))
            .map(|(&p, _)| points.number(p))
            .filter(|&n| n != GONE)
            .collect();
        moved.sort_unstable();
        let mut welded: Vec<(u32, u32)> =
            (self.welded.keys().map(|&p| (p, self.survivor(p)))).collect();
        welded.sort_unstable();
        let total = points.layout.len() as u32;
        let added = points.layout.added() as u32;
        let primitives = match &rows {
            None => {
                let kept = self.base_primitives();
                Numbers {
                    added: kept..kept,
                    base: kept,
                    ..Numbers::default()
                }
            }
            Some(rows) => {
                let total = rows.primitives.len() as u32;
                let added = rows.primitives.added() as u32;
                Numbers {
                    added: total - added..total,
                    removed: rows.removed.clone(),
                    filled: rows.filled.clone(),
                    base: self.base_primitives(),
                    ..Numbers::default()
                }
            }
        };
        let rewired = rows.map(|rows| rows.rewired).unwrap_or_default();
        Change {
            moved,
            points: Numbers {
                added: total - added..total,
                removed: points.gone.clone(),
                welded,
                filled: points.filling.filled.clone(),
                base: self.base_points(),
            },
            primitives,
            rewired,
            topology,
        }
    }
}
