//! Kinogaki Studio's standard geometry values.
//!
//! Studio defines each kind of geometry once, here, and every package works
//! on the same values: kernels operate on them, formats read and write
//! them, render engines draw them, and Studio displays, picks, tabulates
//! and saves them. A kernel adds operations, never a type of its own for
//! the same data.
//!
//! | Value | What |
//! |---|---|
//! | [`Polygons`] | polygons and polylines over shared points ([`Topology`]), with [`Attributes`] |
//! | [`Curves`] | linear, Bézier, B-spline, Catmull-Rom and NURBS curves ([`Basis`]) |
//! | [`Brep`](brep::Brep) | boundary-representation bodies with exact geometry, free curves and datums ([`brep`]) |
//! | [`Attributes`] | named [`Attribute`] columns (an [`AttributeType`]: [`Storage`] × width × [`Meaning`], arrays) and [`Group`]s per [`Domain`]; [`Text`] is indexed; [standard names](names) |
//! | [`Selection`], [`Incidence`] | selected elements of one domain, with the active one; which elements of a value touch which (selection rules live in `kinogaki-edit`) |
//! | [`Triangulation`], [`ShadingNormals`] | how polygons are shown: the one display triangulation and shading rule every viewport, engine and kernel shares |
//! | [`Detail`], [`DisplayQuality`] | how fine geometry is made for a view or a render |
//! | [`Tolerances`] | the project's modeling tolerances |
//! | [`Vec3`], [`Xform`], [`Bounds`], [`Ray`], [`Projective`] | the one vector, placement (affine, column vectors), box, ray and view matrix every crate exchanges; [`Frame`] is an orthonormal frame |
//! | [`Units`], [`Time`], [`Rate`], [`Sampled`] | what a scene unit is; a moment on the tick grid and frames a second; a value across a frame's shutter |
//! | [`parallel`] | the process's shared [`Workers`](parallel::Workers): every parallel step runs on what the host installed |
//!
//! **Validated values.** Every value's fields are private. It is made only
//! through a constructor or builder that checks its rules (each type lists
//! them) and refuses with a [`GeometryError`] naming the element at
//! fault, so code reading a value may rely on them: no invalid value can
//! exist. Enums are `#[non_exhaustive]`, and each says what a reader does
//! with a variant it doesn't know.
//!
//! **Shared and immutable.** Arrays are [`Column`]s: shared buffers that
//! adopt a `Vec` without copying it. A value is never changed in place: an
//! edit (`with_positions`, `with_attributes`, [`Attributes::with`]) makes a
//! new value sharing every column it didn't touch, and checks only what it
//! replaced. Polygons keep their
//! primitives in one shared `Arc<Topology>`, so moving points keeps the
//! topology, its edges and every cache built on it.
//!
//! **Product limits** ([`limits`]): at most 8,388,608 points
//! ([`MAX_POINTS`](limits::MAX_POINTS)), 8,388,608 primitives or curves
//! ([`MAX_PRIMITIVES`](limits::MAX_PRIMITIVES)), 33,554,432 vertices
//! ([`MAX_VERTICES`](limits::MAX_VERTICES)) and names of 256 bytes. Every
//! position component is finite and at most 1e100 in magnitude.
//!
//! # Caches derived from a standard value
//!
//! A kernel or Studio may keep acceleration data (adjacency, half-edges, a
//! triangulation, a BVH), but only as a cache derived from a standard
//! value, never as the value itself:
//!
//! 1. **Keys.** A cache is derived from one shared part of a value and is
//!    keyed by that part's identity: the [`Topology`] for adjacency,
//!    edges, half-edges, triangulation and triangle maps; the positions
//!    [`Column`] for BVHs and bounds; an attribute's `Column` for anything
//!    built from that attribute.
//! 2. **Where caches live.** Use [`Topology::derived`] and
//!    [`Column::derived`]. A slot lives exactly as long as its part and is
//!    shared by every value that shares the part, so a points-only edit
//!    (`with_positions`) keeps every topology cache. Never keep a cache in
//!    a global map keyed by address.
//! 3. **Readers.** Only the crate that defines a cache's type reads it. A
//!    slot is never a channel between plugins.
//! 4. **Never stored.** A cache is never saved, compared or treated as the
//!    value. Dropping it may only cost time.
//! 5. **Key lifetime.** [`Column::key`] and `ptr_eq` are valid while the
//!    part lives, within one session. Use [`Topology::fingerprint`] or a
//!    stamp for anything that outlives it.
//! 6. **Independent caches.** Two kernels' caches over one part are two
//!    slots, and neither sees the other's.
//!
//! The canonical [`Index`] ([`Topology::index`]) is the one cache this
//! crate defines over a topology: every kernel and Studio share its edges
//! and numbering instead of building their own.
//!
//! ```
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicUsize, Ordering};
//! use kinogaki_geometry::{Attributes, Polygons, Topology};
//!
//! /// A kernel's own cache over a topology (its type is its key).
//! struct Valence(Vec<u32>);
//!
//! let t = Topology::new(4, vec![0, 3, 6].into(), vec![0, 1, 2, 0, 2, 3].into(), 2).unwrap();
//! let a = Polygons::new(vec![[0.0; 3]; 4].into(), t, Attributes::new()).unwrap();
//! let builds = AtomicUsize::new(0);
//! let valence = |p: &Polygons| {
//!     p.topology().derived(|t| {
//!         builds.fetch_add(1, Ordering::Relaxed);
//!         Valence((0..t.points()).map(|i| t.index().point_faces()[i].len() as u32).collect())
//!     })
//! };
//! assert_eq!(valence(&a).0, [2, 1, 2, 1]);
//! // A points-only edit shares the topology, and so the cache.
//! let b = a.with_positions(vec![[1.0; 3]; 4].into()).unwrap();
//! assert!(Arc::ptr_eq(&valence(&a), &valence(&b)));
//! assert_eq!(builds.load(Ordering::Relaxed), 1);
//! ```
mod attributes;
mod bounds;
pub mod brep;
mod column;
mod curves;
mod derived;
mod detail;
mod error;
mod frame;
mod incidence;
mod kept;
pub mod limits;
pub mod parallel;
mod polygons;
mod positions;
mod sampled;
mod selection;
mod shading;
mod spline;
mod time;
mod tolerances;
mod topology;
mod triangulation;
mod units;
mod vec3;
mod vector;
mod xform;

pub use attributes::names;
pub use attributes::{
    Attribute, AttributeType, Attributes, Domain, Group, Meaning, Storage, Text, Tuple, Values,
    check_name,
};
pub use bounds::Bounds;
pub use column::Column;
pub use curves::{Basis, CurveDisplay, Curves, CurvesBuilder, Knots};
pub use detail::{Detail, DisplayQuality};
pub use error::{ErrorKind, GeometryError};
pub use frame::Frame;
pub use incidence::Incidence;
pub use polygons::Polygons;
pub use sampled::Sampled;
pub use selection::Selection;
pub use shading::ShadingNormals;
pub use time::{Rate, Time};
pub use tolerances::Tolerances;
pub use topology::{Adjacency, Edge, Index, Topology};
pub use triangulation::{Triangle, Triangulation};
pub use units::{Units, UpAxis};
pub use vec3::{Ray, Vec3};
pub use xform::{Projective, Xform};
