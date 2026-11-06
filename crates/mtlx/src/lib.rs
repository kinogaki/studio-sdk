//! MaterialX 1.39 for Kinogaki Studio: the material layer every engine,
//! format and core share, in our own implementation (MaterialX's C++ library
//! is a reference and a test oracle, never a dependency).
//!
//! - **Documents** ([`Document`]): read from their XML and written back as
//!   MaterialX writes them, comments and blank lines kept; MaterialX 1.36 to
//!   1.38 documents are upgraded to 1.39 as they are read, as MaterialX's own
//!   reader does.
//! - **The library** ([`library`]): MaterialX 1.39.5's definition files
//!   (stdlib, pbrlib, the shading models in bxdf with their translation
//!   graphs, cmlib, nprlib, lights), vendored verbatim as data with their
//!   Apache-2.0 licence (`libraries/`), and Kinogaki's own definitions
//!   (`kinogaki/`), embedded and read once a process.
//! - **Definitions** ([`NodeDef`]): one typed signature of a node each,
//!   their inputs ([`Port`]) with MaterialX's defaults, UI hints, uniform
//!   and enumeration meaning, and the geometric properties
//!   ([`GeomProp`]) unconnected inputs read.
//! - **Expansion** ([`Graph`], [`Library::implementation`]): a node a graph
//!   defines, as that graph, resolved once per definition.
//! - **Translation** ([`Library::translation`]): the chain of
//!   `<from>_to_<to>` definitions from one shading model to another.
//! - **Evaluation** ([`NodeDef::evaluate`]): a node of constants as its value.
//!
//! The crate depends on the standard library alone.
mod document;
mod eval;
mod graph;
mod library;
mod translate;
mod types;
mod upgrade;
mod xml;

pub use document::Document;
pub use graph::{Binding, Graph, GraphNode};
pub use library::{GeomProp, Library, NodeDef, Port, library};
pub use translate::target;
pub use types::{Type, Value};
