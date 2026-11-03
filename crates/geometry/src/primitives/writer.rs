//! [`TableWriter`]: a new table made from a checked one. Kept rows share
//! their cells and aren't checked again, so what an edit costs is what it
//! wrote.
use super::fingerprint::Digests;
use super::{CurveData, Kind, Kinds, Primitives, validate};
use crate::Error;
use crate::column::{Column, Writer};
use crate::limits;
use crate::primitives::Basis;
use std::ops::Range;

/// The rows of a new table, in order: [kept](Self::keep) rows of the table it
/// came from and [written](Self::write) ones, which alone are checked
/// (kept rows hold). [`Primitives::rewrite`] makes one.
pub struct TableWriter<'a> {
    base: &'a Primitives,
    points: usize,
    offsets: Writer<u32>,
    vertices: Writer<u32>,
    /// All one kind while `kinds` stays empty.
    uniform: Option<Kind>,
    kinds: Writer<Kind>,
}

impl Primitives {
    /// A writer of a new table over `points` points (at least this table's
    /// when a row is kept) from this table.
    pub fn rewrite(&self, points: usize) -> TableWriter<'_> {
        TableWriter {
            base: self,
            points,
            offsets: Writer::new(),
            vertices: Writer::new(),
            uniform: self.uniform().or(self.is_empty().then_some(Kind::POLYGON)),
            kinds: Writer::new(),
        }
    }
}

impl TableWriter<'_> {
    /// How many rows it holds.
    pub fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// Whether it holds no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many vertices it holds.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// The base's `rows` next, as they are.
    ///
    /// # Panics
    /// When `rows` are past the base's, or the writer's points are fewer
    /// than the base's (a kept row may name any of them).
    pub fn keep(&mut self, rows: Range<usize>) {
        if rows.is_empty() {
            return;
        }
        assert!(self.points >= self.base.points, "a kept row names a point");
        let base = self.base;
        match &base.kinds {
            Kinds::All(kind) if self.uniform == Some(*kind) => {}
            Kinds::All(kind) => {
                self.mixed();
                (0..rows.len()).for_each(|_| self.kinds.push(*kind));
            }
            Kinds::Each(kinds) => {
                self.mixed();
                self.kinds.keep(kinds, rows.clone());
            }
        }
        let (first, last) = (base.offsets[rows.start], base.offsets[rows.end]);
        let shift = i64::from(self.vertices.len() as u32) - i64::from(first);
        self.vertices
            .keep(&base.vertices, first as usize..last as usize);
        if rows.start == 0 && self.offsets.is_empty() {
            self.offsets.keep(&base.offsets, 0..rows.end + 1);
        } else if shift == 0 && self.offsets.len() == rows.start + 1 {
            self.offsets
                .keep(&base.offsets, rows.start + 1..rows.end + 1);
        } else {
            if self.offsets.is_empty() {
                self.offsets.push(0);
            }
            for o in rows.start + 1..=rows.end {
                self.offsets
                    .push((i64::from(base.offsets[o]) + shift) as u32);
            }
        }
    }

    /// One new row next, of `kind`, checked: rows of curves wait for the
    /// curve data, at the [finish](Self::finish).
    ///
    /// # Errors
    /// The first rule the row breaks.
    pub fn write(&mut self, kind: Kind, corners: &[u32]) -> Result<(), Error> {
        let row = self.len();
        if let Some(&p) = corners.iter().find(|&&p| p as usize >= self.points) {
            return Err(
                Error::invalid(format!("primitive {row} references a missing point {p}"))
                    .element(row),
            );
        }
        if kind.basis() == Basis::Linear
            && let Some(problem) = validate::linear(corners, kind.closed())
        {
            let what = if kind == Kind::POLYGON {
                "polygon"
            } else {
                "polyline"
            };
            return Err(Error::invalid(format!("{what} {row} {problem}")).element(row));
        }
        match self.uniform {
            Some(uniform) if uniform == kind => {}
            _ => {
                self.mixed();
                self.kinds.push(kind);
            }
        }
        if self.offsets.is_empty() {
            self.offsets.push(0);
        }
        self.vertices.extend_from_slice(corners);
        self.offsets.push(self.vertices.len() as u32);
        Ok(())
    }

    /// The kinds become a column, holding the rows so far.
    fn mixed(&mut self) {
        if let Some(uniform) = self.uniform.take() {
            (0..self.len().saturating_sub(self.kinds.len())).for_each(|_| self.kinds.push(uniform));
        }
    }

    /// The new table, `curves` being its curve data when a row is a curve:
    /// only then is every row checked again.
    ///
    /// # Errors
    /// A count past the limits, or the rule a table with curves breaks.
    pub fn finish(mut self, curves: Option<CurveData>) -> Result<Primitives, Error> {
        if self.offsets.is_empty() {
            self.offsets.push(0);
        }
        let (offsets, vertices) = (self.offsets.finish(), self.vertices.finish());
        let kinds = self
            .uniform
            .map_or_else(|| self.kinds.finish(), |_| Column::default());
        let base = self.base;
        limits::check(self.points, limits::MAX_POINTS, "points")?;
        limits::check(offsets.len() - 1, limits::MAX_PRIMITIVES, "primitives")?;
        limits::check(vertices.len(), limits::MAX_VERTICES, "vertices")?;
        if curves.is_some() || base.curves.is_some() {
            let kinds = match self.uniform {
                Some(kind) => Column::new(vec![kind; offsets.len() - 1]),
                None => kinds,
            };
            return Primitives::new(self.points, offsets, vertices, kinds, curves);
        }
        let kinds = match (self.uniform, kinds.first()) {
            (Some(kind), _) => Kinds::All(kind),
            (None, Some(&first)) if kinds.iter().all(|&k| k == first) => Kinds::All(first),
            (None, _) => Kinds::Each(kinds),
        };
        let table = Primitives::unchecked(self.points, offsets, vertices, kinds, None);
        if let Some(digests) = base.digests.get() {
            let _ = table.digests.set(Digests::after(&table, base, digests));
        }
        Ok(table)
    }
}
