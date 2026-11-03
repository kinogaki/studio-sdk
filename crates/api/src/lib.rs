//! Kinogaki Studio's headless plugin API.
//!
//! Studio is an environment: with no packages installed it organises a
//! scene (Parts, Cameras, an empty Material Library, a File node that can
//! read nothing) and nothing more. Everything else (meshes, CAD bodies,
//! file formats, render engines) comes from packages, and this crate is
//! the contract between the two for everything that isn't UI:
//!
//! - **Ids** ([`id`]): declared, interned, one type per kind of thing.
//! - **Data** ([`Data`], [`Typed`]): type-erased shared values on wires
//!   and in object results, read by their kernel through a typed handle.
//! - **Standard data types** ([`standard`], [`geometry`]): the geometry
//!   values every package shares (polygons, curves, attributes,
//!   selections), which core registers, displays and saves; kernels
//!   operate on them.
//! - **Data types** ([`DataType`]): what the environment may ask of a
//!   value it can't look inside (bounds, info, a stamp, saving) and the
//!   optional [`facets`]: [`Displayable`], [`Pickable`], [`Tabular`] and
//!   [`Renderable`]. [`Components`] is a component selection in a type's
//!   own terms.
//! - **Cooking** ([`cook`]): what a node type is and computes ([`NodeType`] with its [`Description`],
//!   [`CookCx`], optional [`Incremental`] cooking); Studio's core owns the
//!   cooking DAG, the caches and the [`parallel`] workers. [`HeldEdit`]s
//!   apply a held node's authored delta to a data type.
//! - **Objects** ([`ObjectKindSpec`], [`ObjectBody`]): kinds of scene
//!   objects a package adds and their package-owned state.
//! - **Networks** ([`network`]): the contexts a package declares
//!   ([`NetworkContext`]) and how a package reads and edits a network,
//!   which core owns ([`NetworkRead`], [`NetworkEdit`]).
//! - **Formats** ([`FileFormat`]): readers and writers of outside files.
//! - **Session slots** ([`Slot`], [`SessionSlot`]): package-owned, undoable
//!   state of an editing session; gestures ([`GestureToken`]), one undo
//!   step over many edits, held by their opener.
//! - **Stored values** ([`Stored`], [`StoredValue`]): package-owned values
//!   the project keeps, one for the project or one per view.
//! - **Icons** ([`icon`]): the named icons a package adds to Studio's
//!   icon set, under its own id.
//! - **Packages** ([`Package`], [`Registry`]): what a package registers,
//!   its UI included ([`PackageUi`]), and the environment's table of it
//!   all, which admits a package only once its checks pass
//!   ([`Registry::check`]); a plugin library exports its package through
//!   [`export_package!`] ([`entry`]).
//!
//! And the plain types those exchange: [`math`] (vectors, placements, units,
//! bounds), [`params`] (parameter values, schemas, diagnostics),
//! [`element`] (the project's element tree codecs write), [`viewport`]
//! (display surfaces, element sets, picks), [`refs`] (the host's objects
//! and nodes), [`render`] (neutral render items), [`time`] (the scene's
//! time) and [`anim`] (a parameter's keys over time).
//!
//! **Self-contained.** The crate depends on the standard library and the
//! SDK's leaf crates alone (the standard geometry values, the render
//! interface, the viewport packet contract), never on Studio, a kernel or any other Kinogaki library:
//! every type it exposes is the SDK's own, and Studio converts between
//! these types and its own libraries' at the boundary. So the SDK builds
//! from a plain clone, and a package written against it compiles against
//! nothing private.
//!
//! Every value crosses as a Rust type: Studio and its packages are built
//! by one compiler and share one copy of each crate, so `TypeId`s and
//! downcasts agree across packages.
pub mod anim;
pub mod cancel;
pub mod codec;
pub mod components;
#[cfg(feature = "testing")]
pub mod conformance;
pub mod cook;
pub mod data;
pub mod data_type;
pub mod element;
pub mod entry;
pub mod error;
pub mod facets;
pub mod format;
pub mod held;
pub mod icon;
pub mod id;
pub mod math;
pub mod mesher;
pub mod network;
pub mod object;
pub mod package;
pub mod parallel;
pub mod params;
pub mod refs;
pub mod registry;
pub mod render;
pub mod session;
pub mod standard;
pub mod stored;
pub mod time;
pub mod viewport;

/// The standard geometry values: what the [`standard`] data types are.
pub use kinogaki_geometry as geometry;

pub use anim::{Interp, Key};
pub use cancel::CancelToken;
pub use codec::{DataCodec, ObjectCodec, ReadCx};
pub use components::Components;
pub use cook::{
    Category, CookCx, CookError, CookPlan, Description, Incremental, Inside, Listing, NewCx,
    NodeRole, NodeType, Outputs, Port, Ports, PortsCx, Prepared, Project, Reads, Resolved, RowsFor,
    SchemaCx,
};
pub use data::{Data, Typed};
pub use data_type::{Color, DataType, InfoRow, Stamp};
pub use element::{ElementReader, ElementWriter, Field, FieldRef};
pub use error::Error;
pub use facets::{
    CellKind, Cells, Combine, DisplayRequest, Displayable, DrawPurpose, Drawn, Look, Materials,
    ObjectPicking, Pickable, Piece, RenderCx, Renderable, Rendered, Role, SnapHit, SnapKind,
    SnapQuery, SnapTargets, Snappable, Table, TableColumn, Tabular, Tally, WorldRay,
};
pub use format::{
    FileFormat, FileItem, FormatOptions, FormatWriter, Frames, Imported, Purpose, Reading, Writing,
};
pub use held::HeldEdit;
pub use icon::{Icon, IconError};
pub use id::{
    ContextId, DataTypeId, FormatId, MesherId, NodeTypeId, ObjectKindId, PackageId, SlotId,
    StoredId,
};
pub use math::{Affine3, Bounds, Mat4, Units, UpAxis, Vec3};
pub use mesher::{Failed, Meshed, Mesher};
pub use network::{NetworkContext, NetworkEdit, NetworkRead};
pub use object::{
    Adoption, CreateCx, KindCreation, MOTION_SAMPLES, ObjectBody, ObjectKindSpec, Processing,
    PublishedMaterial, RenderSetup, RenderSetupKind, RenderedKind, UserSettings, Valued,
};
pub use package::{Package, PackageUi};
pub use parallel::Workers;
pub use params::{Diagnostic, Parameters, Schema, Value};
pub use refs::{NodeRef, ObjectRef};
pub use registry::{Registry, RegistryError};
pub use render::{PlainTriangles, RenderItem, TriangleSource};
pub use session::{GestureMark, GestureToken, SessionSlot, SessionSlots, Slot, SlotDoc};
pub use stored::{Scope, Stored, StoredValue, StoredValues};
pub use time::Time;
pub use viewport::{ElementKind, ElementSet, Hit, Surface, SurfaceSource};
