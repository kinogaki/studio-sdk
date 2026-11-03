//! [`Shape`]: triangles or curves in their own space, with their primvars
//! ([`Attributes`]: Hydra's primvars over the SDK's one typed attribute
//! table), their material assignment ([`Subsets`]) and their placement.
//!
//! A primvar's domain is its interpolation class:
//!
//! | Domain | One value a | How a hit reads it |
//! |---|---|---|
//! | [`Domain::Point`] | point | blended by the hit's barycentrics or along the curve (varying) |
//! | [`Domain::Vertex`] | triangle corner, three a triangle in corner order; or curve control vertex (face-varying) | blended within the triangle or curve segment |
//! | [`Domain::Primitive`] | triangle or curve (uniform) | constant across it |
//! | [`Domain::Detail`] | the whole shape (constant) | the one value |
//!
//! Storage, width and arrays are the attribute's own: nothing narrows on the
//! way, so an I64 `id` arrives exactly; an integer never blends (a hit takes
//! the nearest corner's). Colors ([`Meaning::Color`]) are linear ACEScg.
//!
//! [`Meaning::Color`]: crate::geometry::Meaning::Color
use crate::id::Id;
use crate::prim::{Material, PrimKind, Visibility};
use kinogaki_geometry::names;
use kinogaki_geometry::{Attribute, Attributes, Column, Domain, Error, Primitives, Sampled, Xform};
use std::sync::Arc;

/// The primvars a host always offers an engine, where a shape has them,
/// besides the ones its materials read: shading normals (none: flat), texture
/// coordinates, velocities (`v`, scene units a second) and a curve's width
/// (none: the engine's thinnest).
pub const STANDARD_PRIMVARS: [&str; 4] = [names::N, names::UV, names::V, names::WIDTH];

/// How a shape's points connect.
#[derive(Clone, Debug)]
pub enum Topology {
    /// Three point indices a triangle.
    Triangles(Column<[u32; 3]>),
    /// Curves: every primitive of the table is a polyline or a curve of its
    /// kind ([`Basis`](crate::geometry::Basis)), degree and knots, over the
    /// shape's points. Width is the `width` primvar.
    Curves(Arc<Primitives>),
}

impl Topology {
    /// Its identity: equal keys are the same connectivity.
    pub fn key(&self) -> u64 {
        match self {
            Topology::Triangles(t) => t.key(),
            Topology::Curves(c) => Arc::as_ptr(c) as usize as u64,
        }
    }

    /// The prim kind a shape of it is.
    pub fn kind(&self) -> PrimKind {
        match self {
            Topology::Triangles(_) => PrimKind::Triangles,
            Topology::Curves(_) => PrimKind::Curves,
        }
    }

    /// How many primitives: triangles or curves.
    pub fn primitives(&self) -> usize {
        match self {
            Topology::Triangles(t) => t.len(),
            Topology::Curves(c) => c.len(),
        }
    }

    /// How many vertices: a triangle's three corners, or a curve's control
    /// vertices.
    pub fn vertices(&self) -> usize {
        match self {
            Topology::Triangles(t) => t.len() * 3,
            Topology::Curves(c) => c.vertex_count(),
        }
    }

    fn check(&self, points: usize) -> Result<(), Error> {
        match self {
            Topology::Triangles(t) => {
                let top = t.derived(|t| Highest(t.iter().flatten().copied().max()));
                match top.0 {
                    Some(p) if p as usize >= points => Err(Error::invalid(format_args!(
                        "a triangle names point {p} of {points}"
                    ))),
                    _ => Ok(()),
                }
            }
            Topology::Curves(c) if c.points() != points => Err(Error::invalid(format_args!(
                "curves over {} points for {points} points",
                c.points()
            ))),
            Topology::Curves(c) => match c.polygons().len() {
                0 => Ok(()),
                n => Err(Error::invalid(format_args!("curves holding {n} polygons"))),
            },
        }
    }
}

impl PartialEq for Topology {
    /// Equal connectivity: a table compares by identity, as its value does.
    fn eq(&self, other: &Topology) -> bool {
        match (self, other) {
            (Topology::Triangles(a), Topology::Triangles(b)) => a == b,
            (Topology::Curves(a), Topology::Curves(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// The highest point a triangle column names, kept on it.
struct Highest(Option<u32>);

/// Which material each primitive renders with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Subsets {
    slots: Column<u16>,
    materials: Arc<[Id<Material>]>,
}

impl Subsets {
    /// The slot of a primitive that names no material: the engine's default
    /// renders it.
    pub const DEFAULT: u16 = u16::MAX;

    /// One slot a primitive into `materials`, or [`DEFAULT`](Self::DEFAULT).
    pub fn new(slots: Column<u16>, materials: impl Into<Arc<[Id<Material>]>>) -> Subsets {
        Subsets {
            slots,
            materials: materials.into(),
        }
    }

    /// Each primitive's slot; empty: all take the engine's default.
    pub fn slots(&self) -> &Column<u16> {
        &self.slots
    }

    /// The materials the slots name.
    pub fn materials(&self) -> &Arc<[Id<Material>]> {
        &self.materials
    }

    /// The material primitive `p` renders with (none: the engine's default).
    pub fn of(&self, p: usize) -> Option<Id<Material>> {
        let slot = *self.slots.get(p)?;
        self.materials.get(usize::from(slot)).copied()
    }

    fn check(&self, primitives: usize) -> Result<(), Error> {
        if self.slots.is_empty() {
            return Ok(());
        }
        if self.slots.len() != primitives {
            return Err(Error::invalid(format_args!(
                "{} material slots for {primitives} primitives",
                self.slots.len()
            )));
        }
        let top = self.slots.derived(|s| {
            Highest(
                s.iter()
                    .copied()
                    .filter(|&s| s != Subsets::DEFAULT)
                    .max()
                    .map(u32::from),
            )
        });
        match top.0 {
            Some(slot) if slot as usize >= self.materials.len() => Err(Error::invalid(
                format_args!("a primitive names slot {slot} of {}", self.materials.len()),
            )),
            _ => Ok(()),
        }
    }
}

/// Points, kept on their column: whether all are finite.
struct AllFinite(bool);

/// Triangles or curves in their own space (see the module docs). Made by
/// [`Shape::builder`], changed by [`Shape::rebuilt`].
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    points: Column<[f32; 3]>,
    topology: Topology,
    primvars: Arc<Attributes>,
    subsets: Subsets,
    xform: Sampled<Xform>,
    visibility: Visibility,
    path: Arc<str>,
}

impl Shape {
    /// A shape of `points` connected by `topology`: no primvars, the engine's
    /// default material, at the origin, shown, unnamed.
    pub fn builder(points: Column<[f32; 3]>, topology: Topology) -> ShapeBuilder {
        ShapeBuilder {
            shape: Shape {
                points,
                topology,
                primvars: Arc::default(),
                subsets: Subsets::default(),
                xform: Sampled::still(Xform::IDENTITY),
                visibility: Visibility::SHOWN,
                path: Arc::from(""),
            },
            added: Vec::new(),
        }
    }

    /// A builder starting from this shape: what a host changes a prim
    /// through. What the columns' own checks found stays kept on them, so a
    /// rebuild scans only columns it hasn't seen.
    pub fn rebuilt(&self) -> ShapeBuilder {
        ShapeBuilder {
            shape: self.clone(),
            added: Vec::new(),
        }
    }

    /// Point positions, in the shape's own space.
    pub fn points(&self) -> &Column<[f32; 3]> {
        &self.points
    }

    /// How the points connect.
    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    /// Its primvars, by domain and name: the [standard ones](STANDARD_PRIMVARS)
    /// it has, and what its materials read.
    pub fn primvars(&self) -> &Arc<Attributes> {
        &self.primvars
    }

    /// Primvar `name` and its domain, as Houdini binds one: vertex, point,
    /// primitive, then detail.
    pub fn primvar(&self, name: &str) -> Option<(Domain, &Attribute)> {
        const BINDING: [Domain; 4] = [
            Domain::Vertex,
            Domain::Point,
            Domain::Primitive,
            Domain::Detail,
        ];
        (BINDING.into_iter()).find_map(|d| Some((d, self.primvars.get(d, name)?)))
    }

    /// Which material each primitive renders with.
    pub fn subsets(&self) -> &Subsets {
        &self.subsets
    }

    /// Shape to world across the shutter.
    pub fn xform(&self) -> &Sampled<Xform> {
        &self.xform
    }

    /// Whether it renders, and what sees it.
    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    /// The host's path of it: the name Cryptomatte gives it.
    pub fn path(&self) -> &Arc<str> {
        &self.path
    }

    /// Point `p` at shutter time `time` (frames from the frame) before its
    /// placement, by its velocity when it has one: where deformation blur
    /// sees it, at `rate` frames a second.
    pub fn point_at(&self, p: usize, time: f32, rate: f64) -> [f32; 3] {
        let at = self.points[p];
        let moving = self.primvars.tuples::<[f32; 3]>(Domain::Point, names::V);
        match moving {
            Some(v) => {
                let dt = f64::from(time) / rate;
                std::array::from_fn(|k| at[k] + (f64::from(v[p][k]) * dt) as f32)
            }
            None => at,
        }
    }
}

/// Makes a [`Shape`], checking it whole as it builds: primvars of one value
/// an element of their domain and on no other domain, material slots within
/// the materials, connectivity within the points, finite points.
#[derive(Clone, Debug)]
pub struct ShapeBuilder {
    shape: Shape,
    added: Vec<(Domain, String, Attribute)>,
}

impl ShapeBuilder {
    /// With `points` in place of its points.
    pub fn points(mut self, points: Column<[f32; 3]>) -> Self {
        self.shape.points = points;
        self
    }

    /// With `topology` in place of its topology.
    pub fn topology(mut self, topology: Topology) -> Self {
        self.shape.topology = topology;
        self
    }

    /// With every primitive's material as `subsets` says.
    pub fn subsets(mut self, subsets: Subsets) -> Self {
        self.shape.subsets = subsets;
        self
    }

    /// With `xform` across the shutter.
    pub fn xform(mut self, xform: Sampled<Xform>) -> Self {
        self.shape.xform = xform;
        self
    }

    /// With `visibility`.
    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.shape.visibility = visibility;
        self
    }

    /// Named `path`.
    pub fn path(mut self, path: impl Into<Arc<str>>) -> Self {
        self.shape.path = path.into();
        self
    }

    /// With `primvars` in place of all it holds.
    pub fn primvars(mut self, primvars: impl Into<Arc<Attributes>>) -> Self {
        self.shape.primvars = primvars.into();
        self.added.clear();
        self
    }

    /// With primvar `name` of `domain` too.
    pub fn primvar(mut self, domain: Domain, name: &str, values: Attribute) -> Self {
        self.added.push((domain, name.to_owned(), values));
        self
    }

    /// The shape.
    ///
    /// # Errors
    /// The first rule broken (see the type's docs).
    pub fn build(self) -> Result<Shape, Error> {
        let ShapeBuilder { mut shape, added } = self;
        let points = shape.points.len();
        let finite = |p: &Column<[f32; 3]>| AllFinite(p.iter().flatten().all(|c| c.is_finite()));
        if !shape.points.derived(finite).0 {
            return Err(Error::invalid("a point is not finite"));
        }
        shape.topology.check(points)?;
        shape.subsets.check(shape.topology.primitives())?;
        if !added.is_empty() {
            let mut primvars = (*shape.primvars).clone();
            for (domain, name, values) in added {
                primvars = primvars.with_read(domain, &name, values)?;
            }
            shape.primvars = Arc::new(primvars);
        }
        let count = |domain| match domain {
            Domain::Point => Some(points),
            Domain::Vertex => Some(shape.topology.vertices()),
            Domain::Primitive => Some(shape.topology.primitives()),
            Domain::Detail => Some(1),
            _ => None,
        };
        shape.primvars.check("shapes", None, count)?;
        Ok(shape)
    }
}
