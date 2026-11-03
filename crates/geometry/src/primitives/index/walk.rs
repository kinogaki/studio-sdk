//! Edge loops and rings: walks over an [`Index`], one edge to the next.
use super::Index;

/// No polygon to go on through: a ring ends at it.
const NOWHERE: u32 = u32::MAX;

/// Edges in walking order, as [`Index::edge_loop`] and
/// [`Index::edge_ring`] find them: starting at one end of the walk, or,
/// when `closed`, anywhere on it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Walk {
    /// The edges in order, each once.
    pub edges: Vec<u32>,
    /// Whether the last edge leads back to the first.
    pub closed: bool,
}

impl Index {
    /// The edge loop through edge `e`: the edges continuing it straight
    /// across each point, as far as a straight way on exists.
    ///
    /// - Across a point of four polygons and four edges, a surface edge
    ///   goes on to the edge opposite, sharing no polygon with it.
    /// - A [boundary](Self::is_boundary) edge goes on along the boundary
    ///   while the point has just two boundary edges.
    /// - An edge only lines use goes on while the point has no other edge.
    /// - An edge of three or more polygons goes nowhere: it is its loop.
    ///
    /// ```
    /// use kinogaki_geometry::Primitives;
    ///
    /// // Three quads in a row: the middle line crosses them.
    /// //   4  5  6  7
    /// //   0  1  2  3
    /// let t = Primitives::of_polygons(
    ///     8,
    ///     vec![0, 4, 8, 12].into(),
    ///     vec![0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6].into(),
    /// ).unwrap();
    /// let index = t.index();
    /// let walk = index.edge_loop(t.edge_of(4, 5).unwrap());
    /// // The border goes all the way round.
    /// assert!(walk.closed && walk.edges.len() == 8);
    /// // The rung 1–5 stops at the border on both sides.
    /// let rung = index.edge_loop(t.edge_of(1, 5).unwrap());
    /// assert_eq!((rung.edges.len(), rung.closed), (1, false));
    /// ```
    pub fn edge_loop(&self, e: u32) -> Walk {
        let [low, high] = self.edge(e as usize).points;
        self.walk(e, [high, low], |edge, point| {
            let next = self.straight_on(edge, point)?;
            let mutual = self.straight_on(next, point) == Some(edge);
            mutual.then(|| (next, self.other_end(next, point)))
        })
    }

    /// The edge ring through edge `e`: the edges facing it across each
    /// quad, side by side, as far as quads go on.
    ///
    /// ```
    /// use kinogaki_geometry::Primitives;
    ///
    /// // Three quads in a row, as in [`edge_loop`](Self::edge_loop).
    /// let t = Primitives::of_polygons(
    ///     8,
    ///     vec![0, 4, 8, 12].into(),
    ///     vec![0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6].into(),
    /// ).unwrap();
    /// let index = t.index();
    /// let walk = index.edge_ring(t.edge_of(1, 5).unwrap());
    /// let rungs = [(0, 4), (1, 5), (2, 6), (3, 7)].map(|(a, b)| t.edge_of(a, b).unwrap());
    /// assert_eq!((walk.edges, walk.closed), (rungs.to_vec(), false));
    /// ```
    pub fn edge_ring(&self, e: u32) -> Walk {
        // A walk's state is the polygon it goes on through; the start goes
        // both ways, one through each of its polygons.
        let ways = match *self.edge_faces(e as usize) {
            [first, second] => [second, first],
            [only] => [only, NOWHERE],
            _ => [NOWHERE; 2],
        };
        self.walk(e, ways, |edge, through| {
            let next = self.across(edge, through)?;
            let on = match *self.edge_faces(next as usize) {
                [a, b] if a == through => b,
                [a, _] => a,
                _ => NOWHERE,
            };
            Some((next, on))
        })
    }

    /// Edges from `e` by `step` in both `ways`: `step(edge, state)` gives
    /// the next edge and its state, until it gives none or comes back. A
    /// step is mutual (the next edge's way back is this one), so each edge
    /// has two ways at most and a walk is a path or a cycle through `e`.
    fn walk(&self, e: u32, ways: [u32; 2], step: impl Fn(u32, u32) -> Option<(u32, u32)>) -> Walk {
        let mut forward = vec![e];
        let (mut edge, mut state) = (e, ways[0]);
        while let Some((next, after)) = step(edge, state) {
            if next == e {
                return Walk {
                    edges: forward,
                    closed: true,
                };
            }
            forward.push(next);
            (edge, state) = (next, after);
        }
        let mut edges = Vec::new();
        let (mut edge, mut state) = (e, ways[1]);
        while let Some((next, after)) = step(edge, state) {
            edges.push(next);
            (edge, state) = (next, after);
        }
        edges.reverse();
        edges.append(&mut forward);
        Walk {
            edges,
            closed: false,
        }
    }

    /// The other point of edge `e`.
    fn other_end(&self, e: u32, point: u32) -> u32 {
        let [a, b] = self.edge(e as usize).points;
        if a == point { b } else { a }
    }

    /// The edge going straight on from `edge` across `point`, by the rules
    /// of [`edge_loop`](Self::edge_loop).
    fn straight_on(&self, edge: u32, point: u32) -> Option<u32> {
        let at = point as usize;
        let faces = self.edge_faces(edge as usize);
        match faces.len() {
            0 => {
                let mut others = self.point_every_edge(at).filter(|&o| o != edge);
                let next = others.next()?;
                let only = others.next().is_none() && self.edge_faces(next as usize).is_empty();
                only.then_some(next)
            }
            1 => {
                let mut others =
                    (self.point_edges(at)).filter(|&o| o != edge && self.is_boundary(o as usize));
                let next = others.next()?;
                others.next().is_none().then_some(next)
            }
            2 => {
                let edges: Vec<u32> = self.point_edges(at).collect();
                if edges.len() != 4 || self.point_faces(at).len() != 4 {
                    return None;
                }
                let facing = |o: &u32| {
                    *o != edge
                        && self.edge_faces(*o as usize).len() == 2
                        && !self
                            .edge_faces(*o as usize)
                            .iter()
                            .any(|f| faces.contains(f))
                };
                let mut opposite = edges.into_iter().filter(facing);
                let next = opposite.next()?;
                opposite.next().is_none().then_some(next)
            }
            _ => None,
        }
    }

    /// The side facing `edge` across polygon `through`; none when it is
    /// [`NOWHERE`] or has other than four sides.
    fn across(&self, edge: u32, through: u32) -> Option<u32> {
        if through == NOWHERE {
            return None;
        }
        let sides: Vec<u32> = self.primitive_edges(through as usize).collect();
        let at = sides.iter().position(|&s| s == edge)?;
        (sides.len() == 4).then(|| sides[(at + 2) % 4])
    }
}
