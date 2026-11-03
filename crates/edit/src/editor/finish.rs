//! [`Editor::finish`]: the changes made into a new value, in one pass over
//! what they reach; everything else stays shared.
use super::carry::{carried_attribute, carried_group};
use super::change::{Change, Numbers};
use super::layout::{Blends, Layout, Run, Span};
use super::{Editor, Row};
use kinogaki_geometry::{
    Attributes, Basis, Column, ColumnWriter, CurveData, Domain, Error, Geometry, Kind, Knots,
    Primitives, TableWriter,
};
use std::collections::HashMap;
use std::sync::Arc;

/// No new number: the element is gone.
const GONE: u32 = u32::MAX;

/// The points as the edit has them.
struct Points {
    /// Where each number goes; `None` while none changes.
    renumber: Option<Vec<u32>>,
    /// Which numbers name a point welded away (empty without welds).
    welded: Vec<bool>,
    /// Where the points come from.
    layout: Layout,
    /// The new points' positions, of those that stay.
    added: Vec<[f64; 3]>,
}

impl Points {
    /// `p`'s new number (or [`GONE`]).
    fn number(&self, p: u32) -> u32 {
        match &self.renumber {
            Some(renumber) => renumber[p as usize],
            None => p,
        }
    }
    /// `p`'s new number while it is a point of its own (not gone, not
    /// welded into another).
    fn staying(&self, p: u32) -> Option<u32> {
        let welded = self.welded.get(p as usize).is_some_and(|&w| w);
        Some(self.number(p)).filter(|&n| n != GONE && !welded)
    }
}

/// The table as the edit has it, with where its elements came from.
struct Rows<'a> {
    table: TableWriter<'a>,
    primitives: Layout,
    vertices_from: Layout,
    rewired: Vec<u32>,
    removed: Vec<u32>,
}

/// One corner on its way into a row: its new point and where its values
/// come from.
#[derive(Clone, Copy)]
enum Vertex {
    Kept(u32),
    Blended(Span),
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
        self.remade(structural)
    }

    /// Points moved and nothing else: one copy of the positions.
    fn moved_only(self) -> Result<(Geometry, Change), Error> {
        let mut moved: Vec<(u32, [f64; 3])> = self.moved.into_iter().collect();
        moved.sort_unstable_by_key(|m| m.0);
        let (points, at): (Vec<u32>, Vec<[f64; 3]>) = moved.into_iter().unzip();
        if points.is_empty() {
            return Ok((self.base, Change::default()));
        }
        let value = self.base.with_moved(&points, &at)?;
        let moved = value.moved_points(&self.base);
        let change = Change {
            moved,
            ..Change::default()
        };
        Ok((value, change))
    }

    /// Anything else: what the changes reach is written again.
    fn remade(self, structural: bool) -> Result<(Geometry, Change), Error> {
        let points = self.points();
        let table = self.base.primitives();
        let (primitives, rows) = if structural {
            let mut rows = self.rows(&points)?;
            let curves = self.curves_of(&rows);
            let table = std::mem::replace(&mut rows.table, table.rewrite(0));
            (Arc::new(table.finish(curves)?), Some(rows))
        } else {
            (Arc::new(table.with_points(points.layout.len())?), None)
        };
        let positions = self.positions(&points);
        let attributes = self.attributes(&points, rows.as_ref(), &primitives)?;
        let change = self.change(&points, rows);
        let value = self.base.edited(positions, primitives, attributes)?;
        Ok((value, change))
    }

    /// Which points stay, under what numbers.
    fn points(&self) -> Points {
        let total = self.point_count() as usize;
        let base = self.base_points();
        let renumbers = !self.removed_points.is_empty() || !self.welded.is_empty() || self.loose_go;
        let mut gone = vec![false; if renumbers { total } else { 0 }];
        for &p in self.removed_points.iter().chain(self.welded.keys()) {
            gone[p as usize] = true;
        }
        if self.loose_go {
            let mut named = vec![false; total];
            self.each_row(|row| {
                for &p in row {
                    named[self.survivor(p) as usize] = true;
                }
            });
            for (gone, named) in gone.iter_mut().zip(&named) {
                *gone |= !named;
            }
        }
        let mut layout = Layout::default();
        let mut added = Vec::with_capacity(self.added_points.len());
        let mut renumber = vec![GONE; gone.len()];
        if !renumbers {
            layout.keep(0..base);
        }
        let mut next = 0;
        // Without renumbering only the new points need a look.
        for p in if renumbers { 0 } else { base }..total as u32 {
            if renumbers && gone[p as usize] {
                continue;
            }
            match p.checked_sub(base) {
                None => layout.keep(p..p + 1),
                Some(k) => {
                    layout.add(self.point_from[k as usize]);
                    added.push(self.added_points[k as usize]);
                }
            }
            if renumbers {
                renumber[p as usize] = next;
                next += 1;
            }
        }
        let mut welded = vec![false; if self.welded.is_empty() { 0 } else { total }];
        for &p in self.welded.keys() {
            renumber[p as usize] = renumber[self.survivor(p) as usize];
            welded[p as usize] = true;
        }
        Points {
            renumber: renumbers.then_some(renumber),
            welded,
            layout,
            added,
        }
    }

    /// Calls `f` with every row the result might hold.
    fn each_row(&self, mut f: impl FnMut(&[u32])) {
        for p in 0..self.primitive_count() {
            if !self.removed_primitives.contains(&p) {
                f(self.corners(p));
            }
        }
    }

    /// The table's rows as the edit has them: kept rows share their cells,
    /// the rest are written once each.
    fn rows(&self, points: &Points) -> Result<Rows<'_>, Error> {
        let table = self.base.primitives();
        let base = table.len() as u32;
        let wants_vertices = self.base.attributes().has(Domain::Vertex)
            || (table.curves()).is_some_and(|c| c.weights().is_some());
        let mut rows = Rows {
            table: table.rewrite(points.layout.len()),
            primitives: Layout::default(),
            vertices_from: Layout::default(),
            rewired: Vec::new(),
            removed: Vec::new(),
        };
        // Base rows between the touched ones go in runs.
        let mut touched: Vec<u32> = (self.rewired.keys())
            .chain(self.removed_primitives.iter().filter(|&&p| p < base))
            .copied()
            .collect();
        touched.sort_unstable();
        touched.dedup();
        let mut next = 0;
        for &p in &touched {
            self.keep_rows(&mut rows, next..p, points, wants_vertices)?;
            next = p + 1;
            if self.removed_primitives.contains(&p) {
                rows.removed.push(p);
            } else {
                self.place_written(&mut rows, p, self.rewired[&p], points, wants_vertices)?;
            }
        }
        self.keep_rows(&mut rows, next..base, points, wants_vertices)?;
        for (k, &row) in self.added_rows.iter().enumerate() {
            let number = base + k as u32;
            if self.removed_primitives.contains(&number) {
                rows.removed.push(number);
                continue;
            }
            self.place_written(&mut rows, number, row, points, wants_vertices)?;
        }
        rows.removed.sort_unstable();
        Ok(rows)
    }

    /// `run`'s base rows, as they are: shared when no point changes number.
    fn keep_rows(
        &self,
        rows: &mut Rows<'_>,
        run: std::ops::Range<u32>,
        points: &Points,
        wants_vertices: bool,
    ) -> Result<(), Error> {
        if run.is_empty() {
            return Ok(());
        }
        let table = self.base.primitives();
        let offsets = table.offsets();
        let Some(renumber) = points.renumber.as_deref() else {
            let (first, last) = (offsets[run.start as usize], offsets[run.end as usize]);
            rows.table.keep(run.start as usize..run.end as usize);
            rows.primitives.keep(run);
            if wants_vertices {
                rows.vertices_from.keep(first..last);
            }
            return Ok(());
        };
        let mut new = Vec::new();
        for p in run {
            let corners = table.primitive(p as usize);
            new.clear();
            // Mostly a point changes its number and nothing more.
            let plain = corners.iter().all(|&c| {
                let number = renumber[c as usize];
                new.push(number);
                number != GONE && !points.welded.get(c as usize).is_some_and(|&w| w)
            });
            if !plain {
                let vertices = table.primitive_vertices(p as usize);
                let corners = corners
                    .iter()
                    .zip(vertices)
                    .map(|(&c, v)| (renumber[c as usize], Vertex::Kept(v as u32)));
                if place(rows, table.kind(p as usize), corners, wants_vertices)? {
                    rows.primitives.keep(p..p + 1);
                    rows.rewired.push(rows.primitives.len() as u32 - 1);
                } else {
                    rows.removed.push(p);
                }
                continue;
            }
            rows.table.write(table.kind(p as usize), &new)?;
            rows.primitives.keep(p..p + 1);
            if wants_vertices {
                let range = table.primitive_vertices(p as usize);
                rows.vertices_from
                    .keep(range.start as u32..range.end as u32);
            }
        }
        Ok(())
    }

    /// A row the edit wrote for primitive `number`.
    fn place_written(
        &self,
        rows: &mut Rows<'_>,
        number: u32,
        row: Row,
        points: &Points,
        wants_vertices: bool,
    ) -> Result<(), Error> {
        let range = row.start as usize..(row.start + row.len) as usize;
        let corners = self.corners[range.clone()]
            .iter()
            .zip(&self.corner_from[range])
            .map(|(&point, &span)| {
                let vertex = match self.vertex_blends.of(span) {
                    [(v, weight)] if *weight > 0.0 => Vertex::Kept(*v),
                    _ => Vertex::Blended(span),
                };
                (points.number(point), vertex)
            });
        if !place(rows, row.kind, corners, wants_vertices)? {
            rows.removed.push(number);
            return Ok(());
        }
        match number.checked_sub(self.base_primitives()) {
            None => {
                rows.primitives.keep(number..number + 1);
                rows.rewired.push(rows.primitives.len() as u32 - 1);
            }
            Some(k) => rows.primitives.add(self.primitive_from[k as usize]),
        }
        Ok(())
    }

    /// The curve data of `rows`: degrees and knots by primitive, weights by
    /// vertex; present while a curve is. A row the edit wrote weighs 1.
    fn curves_of(&self, rows: &Rows<'_>) -> Option<CurveData> {
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
            let mut out = Vec::with_capacity(rows.table.vertex_count());
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
        let mut added = points.added.iter();
        for run in layout.runs() {
            match run {
                Run::Kept(range) => out.keep(base, range.start as usize..range.end as usize),
                Run::New(_) => out.push(*added.next().expect("a position a new point")),
            }
        }
        for (&p, &at) in &self.moved {
            if let Some(number) = points.staying(p) {
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
        rows: Option<&Rows<'_>>,
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
                if !edges.is_whole(table.edges().len()) {
                    carry(Domain::Edge, &edges, &Blends::default())?;
                }
            }
        }
        Ok(if changed { Arc::new(out) } else { base.clone() })
    }

    /// The new table's edges by the base's they take their values from:
    /// an edge that stays, or one [`carry_edge`](Editor::carry_edge) names.
    fn edges_layout(&self, new: &Primitives, points: &Points) -> Layout {
        let table = self.base.primitives();
        let number = |p: u32| points.number(p);
        let pair = |a: u32, b: u32| [a.min(b), a.max(b)];
        let mut from: HashMap<[u32; 2], u32> = HashMap::with_capacity(table.edges().len());
        for (e, edge) in table.edges().iter().enumerate() {
            let [a, b] = edge.points.map(number);
            if a != GONE && b != GONE && a != b {
                from.entry(pair(a, b)).or_insert(e as u32);
            }
        }
        for (&to, &source) in &self.edge_from {
            let edge = (source[0] < self.base_points() && source[1] < self.base_points())
                .then(|| table.edge_of(source[0], source[1]))
                .flatten();
            let [a, b] = to.map(number);
            if let Some(edge) = edge
                && a != GONE
                && b != GONE
                && a != b
            {
                from.insert(pair(a, b), edge);
            }
        }
        let mut layout = Layout::default();
        for edge in new.edges() {
            match from.get(&edge.points) {
                Some(&e) => layout.keep(e..e + 1),
                None => layout.add(Span::default()),
            }
        }
        layout
    }

    /// The report.
    fn change(&self, points: &Points, rows: Option<Rows<'_>>) -> Change {
        let base = self.base.positions();
        let mut moved: Vec<u32> = (self.moved.iter())
            .filter(|(p, at)| base[**p as usize].map(f64::to_bits) != at.map(f64::to_bits))
            .filter_map(|(&p, _)| points.staying(p))
            .collect();
        moved.sort_unstable();
        let mut removed: Vec<u32> = (points.renumber.iter().flatten().enumerate())
            .filter(|(p, n)| **n == GONE || points.welded.get(*p).is_some_and(|&w| w))
            .map(|(p, _)| p as u32)
            .collect();
        removed.sort_unstable();
        let mut welded: Vec<(u32, u32)> =
            (self.welded.keys().map(|&p| (p, self.survivor(p)))).collect();
        welded.sort_unstable();
        let total = points.layout.len() as u32;
        let added = points.layout.added() as u32;
        let (primitives, rewired) = match rows {
            None => {
                let kept = self.base_primitives();
                (
                    Numbers {
                        added: kept..kept,
                        ..Numbers::default()
                    },
                    Vec::new(),
                )
            }
            Some(rows) => {
                let total = rows.primitives.len() as u32;
                let added = rows.primitives.added() as u32;
                let mut rewired = rows.rewired;
                rewired.sort_unstable();
                let primitives = Numbers {
                    added: total - added..total,
                    removed: rows.removed,
                    welded: Vec::new(),
                };
                (primitives, rewired)
            }
        };
        Change {
            moved,
            points: Numbers {
                added: total - added..total,
                removed,
                welded,
            },
            primitives,
            rewired,
            topology: true,
        }
    }
}

/// Puts a written or rewritten row into `rows`: corners gone drop,
/// neighbours that became one point collapse to their first, and a row
/// left too short goes. Whether it stayed; the error of the row it broke.
fn place(
    rows: &mut Rows<'_>,
    kind: Kind,
    corners: impl Iterator<Item = (u32, Vertex)>,
    wants_vertices: bool,
) -> Result<bool, Error> {
    let mut row: Vec<(u32, Vertex)> = Vec::new();
    for (point, vertex) in corners {
        if point != GONE && row.last().is_none_or(|last| last.0 != point) {
            row.push((point, vertex));
        }
    }
    if kind.closed() && row.len() > 1 && row[0].0 == row[row.len() - 1].0 {
        row.pop();
    }
    let linear = kind.basis() == Basis::Linear;
    let least = if kind.closed() { 3 } else { 2 };
    if linear && row.len() < least {
        return Ok(false);
    }
    let points: Vec<u32> = row.iter().map(|c| c.0).collect();
    rows.table.write(kind, &points)?;
    for &(_, vertex) in &row {
        if wants_vertices {
            match vertex {
                Vertex::Kept(v) => rows.vertices_from.keep(v..v + 1),
                Vertex::Blended(span) => rows.vertices_from.add(span),
            }
        }
    }
    Ok(true)
}
