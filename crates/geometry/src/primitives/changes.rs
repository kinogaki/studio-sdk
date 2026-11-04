//! [`TableChange`]: which rows of a table differ from another's, at the cost
//! of the cells the two don't share.
use super::{Kinds, Primitives};

/// What differs between a table and an earlier one, for what is numbered by
/// rows and vertices ([`Primitives::changes_from`]). Nothing is known of
/// where rows came from: it compares, so it holds for any two tables, a
/// tool's result or an undo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TableChange {
    /// The rows below [`tail`](Self::tail) whose corners or kind differ,
    /// ascending: each holds the same vertices in both tables.
    pub rewired: Vec<u32>,
    /// The first row whose vertices aren't where the earlier table had them
    /// (or that it doesn't have): from here on every row counts as new.
    pub tail: u32,
}

impl Primitives {
    /// What differs between this table and `before`.
    pub fn changes_from(&self, before: &Primitives) -> TableChange {
        let (was, is) = (&before.offsets, &self.offsets);
        let common = was.len().min(is.len());
        let mut tail = common - 1;
        for range in is.unshared(was) {
            let range = range.start..range.end.min(common);
            if let Some(at) = range.into_iter().find(|&at| was[at] != is[at]) {
                tail = at - 1;
                break;
            }
        }
        let mut rewired = Vec::new();
        let end = is[tail] as usize;
        for range in self.vertices.unshared(&before.vertices) {
            for c in range.start..range.end.min(end) {
                if before.vertices[c] != self.vertices[c] {
                    let row = is.partition_point(|&o| o as usize <= c) as u32 - 1;
                    if rewired.last() != Some(&row) {
                        rewired.push(row);
                    }
                }
            }
        }
        match (&before.kinds, &self.kinds) {
            (Kinds::All(a), Kinds::All(b)) if a == b => {}
            (Kinds::All(_), Kinds::All(_)) => tail = 0,
            _ => {
                rewired.extend(
                    (0..tail as u32)
                        .filter(|&row| before.kind(row as usize) != self.kind(row as usize)),
                );
                rewired.sort_unstable();
                rewired.dedup();
            }
        }
        rewired.retain(|&row| (row as usize) < tail);
        TableChange {
            rewired,
            tail: tail as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::Kind;

    fn table(rows: &[&[u32]]) -> Primitives {
        let mut offsets = vec![0];
        let mut vertices = Vec::new();
        for row in rows {
            vertices.extend_from_slice(row);
            offsets.push(vertices.len() as u32);
        }
        Primitives::of_polygons(99, offsets.into(), vertices.into()).unwrap()
    }

    #[test]
    fn rows_rewired_added_and_shifted_are_told_apart() {
        let a = table(&[&[0, 1, 2], &[1, 2, 3], &[2, 3, 4]]);
        let same = |rows: Vec<u32>, tail| TableChange {
            rewired: rows,
            tail,
        };
        assert_eq!(a.changes_from(&a), same(vec![], 3));
        let b = table(&[&[0, 1, 2], &[1, 2, 9], &[2, 3, 4], &[5, 6, 7]]);
        assert_eq!(b.changes_from(&a), same(vec![1], 3));
        assert_eq!(a.changes_from(&b), same(vec![1], 3));
        // A row of another length moves the vertices after it.
        let c = table(&[&[0, 1, 2, 7], &[1, 2, 3], &[2, 3, 4]]);
        assert_eq!(c.changes_from(&a), same(vec![], 0));
        // A row gone from the middle reads as the next row rewired, the last gone.
        let d = table(&[&[0, 1, 2], &[2, 3, 4]]);
        assert_eq!(d.changes_from(&a), same(vec![1], 2));
    }

    #[test]
    fn a_kind_counts_as_a_rewiring() {
        let kinds = |k: [Kind; 2]| {
            Primitives::new(
                9,
                vec![0, 3, 6].into(),
                vec![0, 1, 2, 2, 3, 4].into(),
                k.to_vec().into(),
                None,
            )
            .unwrap()
        };
        let a = kinds([Kind::POLYGON, Kind::POLYLINE]);
        let b = kinds([Kind::POLYLINE, Kind::POLYLINE]);
        assert_eq!(b.changes_from(&a).rewired, [0]);
        let c = kinds([Kind::POLYLINE, Kind::POLYGON]);
        assert_eq!(c.changes_from(&a).rewired, [0, 1]);
    }
}
