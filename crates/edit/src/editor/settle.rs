//! What an edit comes to, before a column is written: the rows it reaches as
//! they end up, and the numbers of the elements. Removing fills holes: the
//! last kept elements of the base take the numbers of those that went, so
//! every other element keeps its number and its cells, and a removal costs
//! the rows on the elements that moved, whatever the value's size.
use super::layout::{Layout, Span};
use super::{Editor, Row};
use kinogaki_geometry::{Basis, Kind};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

/// No new number: the element is gone.
pub(super) const GONE: u32 = u32::MAX;

/// One corner on its way into a row: where its values come from.
#[derive(Clone, Copy)]
pub(super) enum Vertex {
    Kept(u32),
    Blended(Span),
}

/// A row as the edit ends up with it, in [`Settled::corners`].
#[derive(Clone)]
pub(super) struct SettledRow {
    pub(super) kind: Kind,
    pub(super) corners: Range<usize>,
}

/// The rows the changes reach, as they end up, by their numbers in the edit.
/// Corners name surviving points by their numbers in the edit.
#[derive(Default)]
pub(super) struct Settled {
    /// The rows that stay.
    pub(super) rows: HashMap<u32, SettledRow>,
    pub(super) corners: Vec<(u32, Vertex)>,
    /// The rows that go, ascending.
    pub(super) gone: Vec<u32>,
    /// The base's rows rewired, removed or naming a point gone, ascending.
    pub(super) reached: Vec<u32>,
}

/// Elements of a base value after removals: how many stay, and the kept
/// elements past that count, each with the hole below it that it fills.
pub(super) struct Filling {
    /// How many of the base's elements stay.
    pub(super) kept: u32,
    /// `(element, its new number)`, ascending by both.
    pub(super) filled: Vec<(u32, u32)>,
}

impl Filling {
    /// `base` elements with `gone` (ascending, all below `base`) going.
    pub(super) fn of(base: u32, gone: &[u32]) -> Self {
        let kept = base - gone.len() as u32;
        let (holes, past) = gone.split_at(gone.partition_point(|&g| g < kept));
        let mut going = past.iter().peekable();
        let fillers = (kept..base).filter(|e| going.next_if(|g| *g == e).is_none());
        Self {
            kept,
            filled: fillers.zip(holes.iter().copied()).collect(),
        }
    }

    /// `layout` with the kept elements in their new order.
    pub(super) fn keep(&self, layout: &mut Layout) {
        let mut next = 0;
        for &(element, hole) in &self.filled {
            layout.keep(next..hole);
            layout.keep(element..element + 1);
            next = hole + 1;
        }
        layout.keep(next..self.kept);
    }
}

/// The points as the edit has them.
pub(super) struct Points {
    /// The base's points whose number changes ([`GONE`] for those that go).
    numbers: HashMap<u32, u32>,
    /// Each new point's number.
    added: Vec<u32>,
    /// The base's count.
    base: u32,
    /// Those that stay and where they moved.
    pub(super) filling: Filling,
    /// The points that go, by their numbers in the edit, ascending.
    pub(super) gone: Vec<u32>,
    /// Where the points come from.
    pub(super) layout: Layout,
    /// The new points' positions, of those that stay.
    pub(super) positions: Vec<[f64; 3]>,
}

impl Points {
    /// `p`'s new number ([`GONE`] once removed or welded away).
    pub(super) fn number(&self, p: u32) -> u32 {
        match p.checked_sub(self.base) {
            Some(k) => self.added[k as usize],
            None => self.numbers.get(&p).copied().unwrap_or(p),
        }
    }

    /// Whether a base point went or took another number.
    pub(super) fn renumbered(&self) -> bool {
        self.gone.first().is_some_and(|&g| g < self.base)
    }
}

impl Editor {
    /// The rows the changes reach, as they end up.
    pub(super) fn settle(&self) -> Settled {
        let table = self.base.primitives();
        let base = self.base_primitives();
        let points = self.base_points();
        let mut reached: Vec<u32> = self.rewired.keys().copied().collect();
        reached.extend(self.removed_primitives.iter().filter(|&&p| p < base));
        let gone = (self.removed_points.iter())
            .chain(self.welded.keys())
            .filter(|&&p| p < points);
        for &p in gone {
            reached.extend(table.index().point_primitives(p as usize));
        }
        reached.sort_unstable();
        reached.dedup();
        let mut settled = Settled::default();
        for &p in &reached {
            if self.removed_primitives.contains(&p) {
                settled.gone.push(p);
            } else if let Some(&row) = self.rewired.get(&p) {
                self.settle_written(&mut settled, p, row);
            } else {
                let corners = (table.primitive(p as usize).iter().copied())
                    .zip(table.primitive_vertices(p as usize))
                    .map(|(point, v)| (point, Vertex::Kept(v as u32)));
                self.settle_row(&mut settled, p, table.kind(p as usize), corners);
            }
        }
        settled.reached = reached;
        for (k, &row) in self.added_rows.iter().enumerate() {
            let number = base + k as u32;
            if self.removed_primitives.contains(&number) {
                settled.gone.push(number);
            } else {
                self.settle_written(&mut settled, number, row);
            }
        }
        settled
    }

    /// A row the edit wrote.
    fn settle_written(&self, settled: &mut Settled, number: u32, row: Row) {
        let range = row.start as usize..(row.start + row.len) as usize;
        let corners = (self.corners[range.clone()].iter().copied())
            .zip(self.corner_from[range].iter().copied())
            .map(|(point, span)| {
                let vertex = match self.vertex_blends.of(span) {
                    [(v, weight)] if *weight > 0.0 => Vertex::Kept(*v),
                    _ => Vertex::Blended(span),
                };
                (point, vertex)
            });
        self.settle_row(settled, number, row.kind, corners);
    }

    /// Row `number` of these corners: those gone drop, neighbours that became
    /// one point collapse to their first, and a row left too short goes.
    fn settle_row(
        &self,
        settled: &mut Settled,
        number: u32,
        kind: Kind,
        corners: impl Iterator<Item = (u32, Vertex)>,
    ) {
        let list = &mut settled.corners;
        let start = list.len();
        for (point, vertex) in corners {
            let point = self.survivor(point);
            let repeats = list.len() > start && list[list.len() - 1].0 == point;
            if !repeats && !self.removed_points.contains(&point) {
                list.push((point, vertex));
            }
        }
        if kind.closed() && list.len() > start + 1 && list[start].0 == list[list.len() - 1].0 {
            list.pop();
        }
        let least = if kind.closed() { 3 } else { 2 };
        if kind.basis() == Basis::Linear && list.len() - start < least {
            list.truncate(start);
            settled.gone.push(number);
            return;
        }
        let corners = start..list.len();
        settled.rows.insert(number, SettledRow { kind, corners });
    }

    /// The points the edit leaves unnamed, if it asked: those of the rows it
    /// reached and its new ones. Points no row named before it stay.
    fn loose(&self, settled: &Settled) -> Vec<u32> {
        if !self.loose_go {
            return Vec::new();
        }
        let table = self.base.primitives();
        let points = self.base_points();
        let named: HashSet<u32> = settled.corners.iter().map(|c| c.0).collect();
        let mut candidates: Vec<u32> = (points..self.point_count()).collect();
        for &row in &settled.reached {
            let corners = table.primitive(row as usize).iter();
            candidates.extend(corners.map(|&p| self.survivor(p)));
        }
        candidates.sort_unstable();
        candidates.dedup();
        candidates.retain(|&c| {
            let elsewhere = || {
                c < points
                    && (table.index().point_primitives(c as usize))
                        .any(|row| settled.reached.binary_search(&row).is_err())
            };
            !named.contains(&c) && !elsewhere()
        });
        candidates
    }

    /// Which points stay, under what numbers.
    pub(super) fn points(&self, settled: &Settled) -> Points {
        let base = self.base_points();
        let mut gone = self.loose(settled);
        gone.extend(self.removed_points.iter().chain(self.welded.keys()));
        gone.sort_unstable();
        gone.dedup();
        let of_base = gone.partition_point(|&g| g < base);
        let filling = Filling::of(base, &gone[..of_base]);
        let mut numbers: HashMap<u32, u32> = gone[..of_base].iter().map(|&g| (g, GONE)).collect();
        numbers.extend(filling.filled.iter().copied());
        let mut layout = Layout::default();
        filling.keep(&mut layout);
        let mut going = gone[of_base..].iter().peekable();
        let mut added = Vec::with_capacity(self.added_points.len());
        let mut positions = Vec::with_capacity(self.added_points.len());
        for (k, &at) in self.added_points.iter().enumerate() {
            if going.next_if(|&&g| g == base + k as u32).is_some() {
                added.push(GONE);
            } else {
                added.push(filling.kept + positions.len() as u32);
                layout.add(self.point_from[k]);
                positions.push(at);
            }
        }
        Points {
            numbers,
            added,
            base,
            filling,
            gone,
            layout,
            positions,
        }
    }
}
