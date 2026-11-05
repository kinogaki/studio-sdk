//! Kinogaki Studio's edit contracts: what core and the kernels agree on
//! when a value is selected and changed.
//!
//! Core owns the geometric values (`kinogaki-geometry`); kernels bring
//! the operations and the math; plugins bring tools and nodes. The edit
//! policy core and every kernel share lives here, beside the data crate
//! rather than in it; a kernel reaches it through `kinogaki-studio-api`
//! (`kinogaki_studio_api::edit`), where its verbs are declared:
//!
//! | Module | What |
//! |---|---|
//! | [`select`] | a selection as another domain ([`convert`](select::convert), by [`Coverage`](select::Coverage)), how a pick grows ([`Mode`](select::Mode): a domain and an [`Expand`](select::Expand)), and what the selection commands find from it (loops, rims, borders, likeness, the [shortest way](select::Ways)) |
//! | [`deform`] | a move as data ([`Deform`](deform::Deform): a [`Move`](deform::Move), per-point weights and a [`Symmetry`](deform::Symmetry)), the one function a preview and a commit place points with |
//! | [`editor`] | the one way a stored value changes ([`Editor`](editor::Editor)): adding, removing, welding, splitting and rewiring, attributes, groups and ids following, unchanged columns shared, with a report of what changed ([`Change`](editor::Change)); values [gathered](editor::gather) into one |
//! | [`ids`] | stable ids: [`fresh`](ids::fresh) ones for new elements |
//! | [`preview`] | the points a drag moves ([`Moving`](preview::Moving)), which core previews over the shown value without a cook |
//!
//! Every rule reads values through `kinogaki-geometry`'s
//! [`Incidence`](kinogaki_geometry::Incidence), so one rule serves
//! polygons and B-rep alike.
pub mod deform;
pub mod editor;
pub mod ids;
pub mod preview;
pub mod select;
