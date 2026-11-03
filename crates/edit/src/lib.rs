//! Kinogaki Studio's edit contracts: what core and the kernels agree on
//! when a value is selected and changed.
//!
//! Core owns the geometric values (`kinogaki-geometry`); kernels bring
//! the operations and the math; plugins bring tools and nodes. A kernel
//! depends on `kinogaki-geometry` and this crate alone, so the edit
//! policy core and every kernel share lives here instead of in the data
//! crate:
//!
//! | Module | What |
//! |---|---|
//! | [`select`] | a selection as another domain ([`convert`](select::convert), by [`Coverage`](select::Coverage)), and how a pick grows ([`Mode`](select::Mode): a domain and an [`Expand`](select::Expand)) |
//! | [`deform`] | a move as data ([`Deform`](deform::Deform): a [`Move`](deform::Move), per-point weights and a [`Symmetry`](deform::Symmetry)), the one function a preview and a commit place points with |
//! | [`ids`] | stable ids: [`fresh`](ids::fresh) ones for new elements, and how a Merge keeps them ([`merged`](ids::merged)) |
//! | [`preview`] | the points a drag moves ([`Moving`](preview::Moving)), which core previews over the shown value without a cook |
//!
//! Every rule reads values through `kinogaki-geometry`'s
//! [`Incidence`](kinogaki_geometry::Incidence), so one rule serves
//! polygons and B-rep alike.
pub mod deform;
pub mod ids;
pub mod preview;
pub mod select;
