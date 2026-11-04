//! [`Editor`]: the one way a stored [`Geometry`] changes. Core owns what is
//! stored and how; a tool or a kernel asks for the change here and supplies
//! only the operation's logic: which points, which corners, from what.
//!
//! An editor is a list of changes over a shared value, and
//! [`finish`](Editor::finish) makes the new value of them:
//!
//! - **Unchanged columns stay shared.** Moving points copies the positions
//!   alone and keeps the table, its index and caches. Adding primitives
//!   leaves every Point attribute alone. Only what a change reaches is
//!   rewritten, once, in one pass, however many changes led to it.
//! - **Attributes follow.** A new element names its sources, `(element,
//!   weight)` of the same domain, and every attribute takes its value by the
//!   [`rule`] of its type: a mean for reals, normalized for normals, along
//!   the arc for rotations, the strongest source's for integers, text and
//!   arrays. Without sources it takes the type's default. A removed element's
//!   values drop, and the `id` attribute keeps kept elements' ids and gives
//!   new ones [fresh](crate::ids::fresh) ids.
//! - **Groups follow.** Kept elements stay in their groups and a new one
//!   joins its strongest source's. A selection made on the old value finds
//!   its elements through their ids
//!   ([`reresolve`](crate::select::reresolve)).
//! - **It reports what it did** ([`Change`]), so a display patches what moved
//!   and a tool knows where its elements ended up.
//! - **Values gather** ([`gather`]): several values into one, each at a
//!   placement, by the same attribute rules.
//!
//! Numbers stay put during an edit: a base element keeps its number, a new
//! one takes the next, and a removal takes effect at the finish: the last
//! kept elements of the base fill the holes, so every other element keeps its
//! number and a removal costs what it reaches, and the new ones follow
//! ([`Numbers::of`] says where a number went). Sources always mean the base
//! value's data, whatever else the same edit changes. A bad number is
//! remembered and reported by [`finish`](Editor::finish), so a tool's loops
//! stay plain.
//!
//! ```
//! use kinogaki_edit::editor::{Corner, Editor};
//! use kinogaki_geometry::{Attributes, Geometry, Kind, Primitives};
//! use std::sync::Arc;
//!
//! let t = Primitives::of_polygons(4, vec![0, 4].into(), vec![0, 1, 2, 3].into()).unwrap();
//! let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
//! let square = Geometry::new(positions.into(), t, Attributes::new()).unwrap();
//!
//! // A triangle on the side 1–2, with a point halfway out.
//! let mut edit = Editor::new(&square);
//! let tip = edit.add_point([2.0, 0.5, 0.0], &[(1, 0.5), (2, 0.5)]);
//! let corners = [1, tip, 2].map(|point| Corner { point, from: &[] });
//! let triangle = edit.add_primitive(Kind::POLYGON, &corners, &[(0, 1.0)]);
//! let (house, change) = edit.finish().unwrap();
//! assert_eq!((house.count(kinogaki_geometry::Domain::Point), triangle), (Some(5), 1));
//! assert_eq!((change.points.added.clone(), change.primitives.added.clone()), (4..5, 1..2));
//!
//! // Points alone: the table and all it caches stay the value's.
//! let mut edit = Editor::new(&house);
//! edit.set_position(tip, [3.0, 0.5, 0.0]);
//! let (longer, change) = edit.finish().unwrap();
//! assert!(Arc::ptr_eq(longer.primitives(), house.primitives()));
//! assert_eq!((change.moved, change.topology), (vec![tip], false));
//! ```
mod carry;
mod change;
mod finish;
mod gather;
mod layout;
mod settle;
mod split;

pub use carry::{Rule, rule};
pub use change::{Change, Numbers};
pub use gather::{Placed, gather, gather_table, spread};

use crate::deform::Deform;
use kinogaki_geometry::{Basis, Error, Geometry, Kind};
use layout::{Blends, Source, Span};
use std::collections::{HashMap, HashSet};

/// One corner of a new or rewired primitive: the point it names, and the
/// base value's vertices its Vertex attributes come from.
#[derive(Clone, Copy, Debug)]
pub struct Corner<'a> {
    /// The point.
    pub point: u32,
    /// Vertices of the base value and their weights.
    pub from: &'a [(u32, f64)],
}

/// A primitive's row as the edit has it, in the editor's flat lists.
#[derive(Clone, Copy, Debug)]
struct Row {
    kind: Kind,
    start: u32,
    len: u32,
}

/// Changes to one shared [`Geometry`] ([the module](self) says how).
#[derive(Debug)]
pub struct Editor {
    base: Geometry,
    /// The first fault: what [`finish`](Self::finish) reports.
    fault: Option<Error>,
    /// Base points' new positions.
    moved: HashMap<u32, [f64; 3]>,
    added_points: Vec<[f64; 3]>,
    /// Each added point's sources.
    point_from: Vec<Span>,
    removed_points: HashSet<u32>,
    /// Points welded away and the points they became.
    welded: HashMap<u32, u32>,
    /// Points no primitive names go at the finish.
    loose_go: bool,
    /// Base primitives' rewritten rows.
    rewired: HashMap<u32, Row>,
    added_rows: Vec<Row>,
    /// Each added primitive's sources.
    primitive_from: Vec<Span>,
    removed_primitives: HashSet<u32>,
    /// The rows' corners, end to end.
    corners: Vec<u32>,
    /// Each corner's sources, as `corners`.
    corner_from: Vec<Span>,
    /// The sources of every new element, by domain.
    point_blends: Blends,
    primitive_blends: Blends,
    vertex_blends: Blends,
    /// The base edge (as its points) each new edge (as its points, low
    /// first) takes its Edge attributes from, while the value has any.
    edge_from: HashMap<[u32; 2], [u32; 2]>,
    /// The edited primitives naming each point, the rows the base index
    /// doesn't know, once a split asks. A row rewired away stays listed;
    /// readers check.
    edited_through: Option<HashMap<u32, Vec<u32>>>,
}

impl Editor {
    /// An editor over `base`, sharing it. Nothing is read or copied until
    /// a change asks.
    pub fn new(base: &Geometry) -> Self {
        Self {
            base: base.clone(),
            fault: None,
            moved: HashMap::new(),
            added_points: Vec::new(),
            point_from: Vec::new(),
            removed_points: HashSet::new(),
            welded: HashMap::new(),
            loose_go: false,
            rewired: HashMap::new(),
            added_rows: Vec::new(),
            primitive_from: Vec::new(),
            removed_primitives: HashSet::new(),
            corners: Vec::new(),
            corner_from: Vec::new(),
            point_blends: Blends::default(),
            primitive_blends: Blends::default(),
            vertex_blends: Blends::default(),
            edge_from: HashMap::new(),
            edited_through: None,
        }
    }

    fn base_points(&self) -> u32 {
        self.base.primitives().points() as u32
    }

    fn base_primitives(&self) -> u32 {
        self.base.primitives().len() as u32
    }

    /// How many points are numbered: the base's and the new ones.
    pub fn point_count(&self) -> u32 {
        self.base_points() + self.added_points.len() as u32
    }

    /// How many primitives are numbered: the base's and the new ones.
    pub fn primitive_count(&self) -> u32 {
        self.base_primitives() + self.added_rows.len() as u32
    }

    /// Point `p`'s position as the edit has it.
    ///
    /// # Panics
    /// When `p` is not a point.
    pub fn position(&self, p: u32) -> [f64; 3] {
        match p.checked_sub(self.base_points()) {
            Some(k) => self.added_points[k as usize],
            None => (self.moved.get(&p).copied()).unwrap_or(self.base.positions()[p as usize]),
        }
    }

    /// Primitive `p`'s corners as the edit has them (welds show at the
    /// finish).
    ///
    /// # Panics
    /// When `p` is not a primitive.
    pub fn corners(&self, p: u32) -> &[u32] {
        match self.row(p) {
            Some(row) => &self.corners[row.start as usize..(row.start + row.len) as usize],
            None => self.base.primitives().primitive(p as usize),
        }
    }

    /// Primitive `p`'s kind.
    ///
    /// # Panics
    /// When `p` is not a primitive.
    pub fn kind(&self, p: u32) -> Kind {
        match self.row(p) {
            Some(row) => row.kind,
            None => self.base.primitives().kind(p as usize),
        }
    }

    /// The row the edit keeps for primitive `p`; none while it is the
    /// base's.
    fn row(&self, p: u32) -> Option<Row> {
        match p.checked_sub(self.base_primitives()) {
            Some(k) => Some(self.added_rows[k as usize]),
            None => self.rewired.get(&p).copied(),
        }
    }

    /// The point `p` ends as, through its welds.
    fn survivor(&self, mut p: u32) -> u32 {
        while let Some(&keep) = self.welded.get(&p) {
            p = keep;
        }
        p
    }

    /// Remembers the first fault.
    fn refuse(&mut self, why: String) {
        self.fault.get_or_insert(Error::invalid(why));
    }

    /// Whether `p` is a numbered point, remembering a fault if not.
    fn check_point(&mut self, p: u32) -> bool {
        let known = p < self.point_count();
        if !known {
            self.refuse(format!(
                "point {p} is past the last of {}",
                self.point_count()
            ));
        }
        known
    }

    /// Whether `p` is a numbered primitive, remembering a fault if not.
    fn check_primitive(&mut self, p: u32) -> bool {
        let known = p < self.primitive_count();
        if !known {
            let last = self.primitive_count();
            self.refuse(format!("primitive {p} is past the last of {last}"));
        }
        known
    }

    /// Sets point `p`'s position (a new point's too).
    pub fn set_position(&mut self, p: u32, at: [f64; 3]) {
        if !self.check_point(p) {
            return;
        }
        match p.checked_sub(self.base_points()) {
            Some(k) => self.added_points[k as usize] = at,
            None => {
                self.moved.insert(p, at);
            }
        }
    }

    /// `points` placed by `deform`, each by its weight (in `points`' order):
    /// the commit of a drag the viewport previewed
    /// ([`Moving`](crate::preview::Moving)), through the one function the
    /// preview places points with, from where the edit has them.
    pub fn deform(&mut self, points: &[u32], deform: &Deform) {
        for (i, &p) in points.iter().enumerate() {
            if !self.check_point(p) {
                return;
            }
            let weight = deform.weights.as_ref().and_then(|w| w.get(i)).copied();
            let at = deform.place(self.position(p).into(), weight.unwrap_or(1.0));
            self.set_position(p, at.to_array());
        }
    }

    /// A new point at `at`, its attributes from the points `from`; its
    /// number.
    pub fn add_point(&mut self, at: [f64; 3], from: &[(u32, f64)]) -> u32 {
        let base = self.base_points();
        let span = self.point_blends.add(from, base, &self.point_from);
        let Some(span) = span else {
            self.refuse(format!("a new point's sources are bad: {from:?}"));
            return self.point_count();
        };
        self.added_points.push(at);
        self.point_from.push(span);
        self.point_count() - 1
    }

    /// Removes point `p` and every corner naming it, as Houdini's points
    /// go: a primitive left with too few corners goes too.
    pub fn remove_point(&mut self, p: u32) {
        if self.check_point(p) {
            self.removed_points.insert(p);
        }
    }

    /// At the finish, removes the points this edit leaves unnamed: those of
    /// the rows it removed or changed, and its new ones. Points no row named
    /// before it stay; finding those is a tool's choice to scan for them.
    pub fn remove_loose_points(&mut self) {
        self.loose_go = true;
    }

    /// Makes point `gone` one with point `keep`, which keeps its position,
    /// attributes and id. Corners naming `gone` name `keep`; a primitive's
    /// neighbouring corners that become one point collapse to one (their
    /// first), and a primitive left with too few corners goes. A polygon
    /// left repeating a point elsewhere is refused at the finish.
    pub fn weld(&mut self, keep: u32, gone: u32) {
        if !self.check_point(keep) || !self.check_point(gone) {
            return;
        }
        let (keep, gone) = (self.survivor(keep), self.survivor(gone));
        if keep != gone {
            self.welded.insert(gone, keep);
        }
    }

    /// A new primitive of `kind` (a polygon or a polyline), its attributes
    /// from the primitives `from`; its number. A curve lands with a kernel
    /// that makes them.
    pub fn add_primitive(
        &mut self,
        kind: Kind,
        corners: &[Corner<'_>],
        from: &[(u32, f64)],
    ) -> u32 {
        if kind.basis() != Basis::Linear {
            self.refuse("the editor adds polygons and polylines".into());
            return self.primitive_count();
        }
        let base = self.base_primitives();
        let span = self.primitive_blends.add(from, base, &self.primitive_from);
        let Some(span) = span else {
            self.refuse(format!("a new primitive's sources are bad: {from:?}"));
            return self.primitive_count();
        };
        let number = self.primitive_count();
        let Some(row) = self.write_row(kind, number, corners) else {
            return number;
        };
        self.added_rows.push(row);
        self.primitive_from.push(span);
        number
    }

    /// Primitive `p`'s corners replaced by `corners`: the point each names
    /// and what its Vertex attributes come from. A corner's own vertex as
    /// its source keeps its values, unchanged and shared; more or fewer
    /// corners are the way to add and remove vertices.
    pub fn rewire(&mut self, p: u32, corners: &[Corner<'_>]) {
        if !self.check_primitive(p) {
            return;
        }
        let kind = self.kind(p);
        if let Some(row) = self.write_row(kind, p, corners) {
            self.set_row(p, row);
        }
    }

    /// Removes primitive `p`; its points stay
    /// ([`remove_loose_points`](Self::remove_loose_points)).
    pub fn remove_primitive(&mut self, p: u32) {
        if self.check_primitive(p) {
            self.removed_primitives.insert(p);
        }
    }

    /// Declares that the new edge between points `to` takes its Edge
    /// attributes and groups from the edge between points `from` (of the
    /// base value, or itself declared). Unnecessary for an edge that stays
    /// (its points' numbers may change; its values follow) and without Edge
    /// attributes it costs nothing.
    pub fn carry_edge(&mut self, to: [u32; 2], from: [u32; 2]) {
        if !self.base.attributes().has(kinogaki_geometry::Domain::Edge) {
            return;
        }
        let pair = |[a, b]: [u32; 2]| [a.min(b), a.max(b)];
        let from = pair(from);
        let source = self.edge_from.get(&from).copied().unwrap_or(from);
        self.edge_from.insert(pair(to), source);
    }

    /// Appends a row, its corners' sources kept.
    fn write_row(&mut self, kind: Kind, p: u32, corners: &[Corner<'_>]) -> Option<Row> {
        let start = self.corners.len() as u32;
        let vertices = self.base.primitives().vertex_count() as u32;
        for corner in corners {
            if !self.check_point(corner.point) {
                return None;
            }
            let Some(span) = self.vertex_blends.add(corner.from, vertices, &[]) else {
                self.refuse(format!(
                    "primitive {p}: a corner's sources are bad: {:?}",
                    corner.from
                ));
                return None;
            };
            self.push_corner(p, corner.point, span);
        }
        Some(Row {
            kind,
            start,
            len: corners.len() as u32,
        })
    }

    /// One corner of primitive `p`'s row being written.
    fn push_corner(&mut self, p: u32, point: u32, from: Span) {
        self.corners.push(point);
        self.corner_from.push(from);
        if let Some(through) = &mut self.edited_through {
            through.entry(point).or_default().push(p);
        }
    }

    /// Primitive `p`'s row is now `row`.
    fn set_row(&mut self, p: u32, row: Row) {
        match p.checked_sub(self.base_primitives()) {
            Some(k) => self.added_rows[k as usize] = row,
            None => {
                self.rewired.insert(p, row);
            }
        }
    }

    /// Primitive `p`'s corners with their sources: the row the edit keeps,
    /// or the base's, each corner from its own vertex.
    fn corners_with_sources(&mut self, p: u32) -> Vec<(u32, Span)> {
        if let Some(row) = self.row(p) {
            let range = row.start as usize..(row.start + row.len) as usize;
            return (self.corners[range.clone()].iter().copied())
                .zip(self.corner_from[range].iter().copied())
                .collect();
        }
        let table = self.base.primitives().clone();
        let vertices = table.primitive_vertices(p as usize);
        let vertex_count = table.vertex_count() as u32;
        (table.primitive(p as usize).iter().zip(vertices))
            .map(|(&point, v)| {
                let span = self
                    .vertex_blends
                    .add(&[(v as u32, 1.0)], vertex_count, &[]);
                (point, span.expect("a base vertex at full weight"))
            })
            .collect()
    }

    /// Appends a row from corners already holding their sources.
    fn write_spans(&mut self, kind: Kind, p: u32, corners: &[(u32, Span)]) -> Row {
        let start = self.corners.len() as u32;
        for &(point, span) in corners {
            self.push_corner(p, point, span);
        }
        Row {
            kind,
            start,
            len: corners.len() as u32,
        }
    }
}

/// Sources in the editor's internal form, for its submodules.
fn one(element: u32) -> [Source; 1] {
    [(element, 1.0)]
}
