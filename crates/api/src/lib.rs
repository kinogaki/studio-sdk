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
//!   Both are the value model's ([`value`]), which every parameter, render
//!   setting and material port holds its [`Value`] in.
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
//!   cooking DAG, the caches and the workers ([`geometry::parallel`]). [`HeldEdit`]s
//!   apply a held node's authored delta to a data type.
//! - **Kernels** ([`kernel`]): heavy algorithm libraries one package offers
//!   the others as verbs, reached through core while they are loaded.
//! - **Scripts** ([`script`]): code a node runs over a standard value on the
//!   host's script engine, reached through core.
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
//! And the plain types those exchange: the geometry crate's one vocabulary
//! ([`Vec3`], [`Xform`], [`Bounds`], [`Units`], [`Time`] and [`Rate`], and
//! the one [`Error`] every fallible call returns, its [`Diagnostic`] what
//! the user reads), [`params`] (parameter values and schemas), [`element`] (the
//! project's element tree codecs write), [`viewport`] (display surfaces,
//! element sets, picks), [`refs`] (the host's objects and nodes),
//! [`render`] (neutral render items) and [`anim`] (a parameter's keys over
//! time).
//!
//! **Self-contained.** The crate depends on the standard library and the
//! SDK's leaf crates alone (the value model, the standard geometry values,
//! the render interface, the viewport packet contract), never on Studio, a kernel or any other Kinogaki library:
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
pub mod color;
pub mod components;
pub mod cook;
pub mod data_type;
pub mod element;
pub mod entry;
pub mod facets;
pub mod format;
pub mod held;
pub mod icon;
pub mod image_files;
pub mod kernel;
pub mod mesher;
pub mod network;
pub mod object;
pub mod package;
pub mod params;
pub mod refs;
pub mod registry;
pub mod render;
pub mod script;
pub mod session;
pub mod standard;
pub mod stored;
pub mod viewport;

/// The edit contracts core and the kernels share: selection rules and
/// modes, deformations, a drag's moving points.
pub use kinogaki_edit as edit;
/// The standard geometry values: what the [`standard`] data types are.
pub use kinogaki_geometry as geometry;
/// The SDK's one vocabulary of vectors, placements, boxes, units, time and
/// errors, which every crate exchanges: the standard geometry values' own.
pub use kinogaki_geometry::{
    Bounds, Diagnostic, Error, ErrorKind, Rate, Severity, TextSpan, Time, Units, UpAxis, Vec3,
    Xform,
};
/// The value model: declared ids, data values, and the one [`Value`] and
/// [`ValueKind`](params::ValueKind) parameters, render settings and
/// material ports hold.
pub use kinogaki_value as value;
pub use kinogaki_value::{data, id};

pub use anim::{Interp, Key};
pub use cancel::CancelToken;
pub use codec::{DataCodec, ObjectCodec, ReadCx};
pub use components::Components;
pub use cook::{
    Category, CookCx, CookPlan, Description, Incremental, Inside, Iteration, Listing, NewCx,
    NodeRole, NodeType, Outputs, Pass, Passes, Port, Ports, PortsCx, Prepared, Project, Reads,
    Resolved, RowsFor, SchemaCx,
};
pub use data::{Data, Typed};
pub use data_type::{Color, DataType, InfoRow, Stamp, stamp_of};
pub use element::{ElementReader, ElementWriter, Field, FieldRef};
pub use facets::{
    Canvas, CanvasPiece, CellKind, Cells, Combine, DisplayRequest, Displayable, DrawPurpose, Drawn,
    Look, Materials, ObjectPicking, Pickable, Picture, Piece, Probe, RenderCx, Renderable,
    Rendered, Role, SnapHit, SnapKind, SnapQuery, SnapTargets, Snappable, Table, TableColumn,
    Tabular, Tally,
};
pub use format::{
    FileFormat, FileItem, FormatOptions, FormatWriter, Frames, Imported, Purpose, Reading, Writing,
};
pub use held::HeldEdit;
pub use icon::Icon;
pub use id::{
    ContextId, CreationId, DataTypeId, FormatId, MesherId, NodeTypeId, ObjectKindId, PackageId,
    SlotId, StoredId, VerbId,
};
pub use mesher::{Meshed, Mesher};
pub use network::{NetworkContext, NetworkEdit, NetworkRead};
pub use object::{
    Adoption, CreateCx, KindCreation, ObjectBody, ObjectKindSpec, Processing, PublishedMaterial,
    RenderColor, RenderSetup, RenderSetupKind, RenderedKind, Valued,
};
pub use package::{Package, PackageUi};
pub use params::{Parameters, Schema, Value};
pub use refs::{NodeRef, ObjectRef};
pub use registry::Registry;
pub use render::{PlainTriangles, RenderItem, ShapeSource};
pub use session::{GestureMark, GestureToken, SessionSlot, SessionSlots, Slot, SlotDoc};
pub use stored::{Scope, Stored, StoredValue, StoredValues};
pub use viewport::{ElementKind, ElementSet, Hit, Surface, SurfaceSource};
