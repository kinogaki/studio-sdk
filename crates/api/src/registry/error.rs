//! [`RegistryError`]: why a package was refused.
use crate::id::{ContextId, ConversionId, DataTypeId, NodeTypeId, PackageId};
use std::fmt;
use std::sync::Arc;

/// Why a package was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// A package with this id is already registered.
    DuplicatePackage(PackageId),
    /// `package` declares `id` (a `what`), which `first` already declared
    /// (or `package` declares twice).
    DuplicateId {
        /// The refused package.
        package: PackageId,
        /// What kind of id ("data type", "format").
        what: &'static str,
        /// The id.
        id: &'static str,
        /// The package that declared it first.
        first: PackageId,
    },
    /// `package` declares a creation named `id`, which isn't a well-formed
    /// id ([`well_formed`](crate::id::well_formed)).
    MalformedId {
        /// The refused package.
        package: PackageId,
        /// The text.
        id: String,
    },
    /// `package` declares `id`, which is under another vendor's name.
    ForeignId {
        /// The refused package.
        package: PackageId,
        /// The id.
        id: &'static str,
    },
    /// `by` (an id `package` declares) names data type `data`, which no
    /// package registered before it.
    UnknownDataType {
        /// The refused package.
        package: PackageId,
        /// What names the type.
        by: &'static str,
        /// The missing type.
        data: DataTypeId,
    },
    /// `package` declares how deltas of type `delta` apply, which `first`
    /// already declares.
    DuplicateHeldEdit {
        /// The refused package.
        package: PackageId,
        /// The data type of the deltas.
        delta: DataTypeId,
        /// The package that declared it first.
        first: PackageId,
    },
    /// `package` declares a second conversion from `from` to `to`; `first`
    /// is the one already registered.
    DuplicateConversion {
        /// The refused package.
        package: PackageId,
        /// The type converted from.
        from: DataTypeId,
        /// The type converted to.
        to: DataTypeId,
        /// The conversion registered first.
        first: ConversionId,
    },
    /// `package` brings a compiler for `context`, which neither it nor a
    /// registered package declares as a compiled context.
    StrayCompiler {
        /// The refused package.
        package: PackageId,
        /// The context.
        context: ContextId,
    },
    /// `package`'s node type `node` declares parameter `parameter`, whose
    /// name starts with [`SPARE`](crate::cook::SPARE), which spares take.
    SpareName {
        /// The refused package.
        package: PackageId,
        /// The node type.
        node: NodeTypeId,
        /// The parameter.
        parameter: String,
    },
    /// `package` brings a runner for `context`, which neither it nor a
    /// registered package declares as a compiled context.
    StrayRunner {
        /// The refused package.
        package: PackageId,
        /// The context.
        context: ContextId,
    },
    /// `package`'s node type `node` holds a network of `context` inside
    /// it, which neither it nor a registered package declares.
    UnknownContext {
        /// The refused package.
        package: PackageId,
        /// The node type.
        node: NodeTypeId,
        /// The context.
        context: ContextId,
    },
    /// `package`'s node type `node` closes a loop of contexts holding each
    /// other's networks: `path` runs from the context the node is offered
    /// in, through each held, back to it.
    InsideCycle {
        /// The refused package.
        package: PackageId,
        /// The node type.
        node: NodeTypeId,
        /// The loop, its first context again at its end.
        path: Vec<ContextId>,
    },
    /// `package` brings a compiler for `context`, which `first` already
    /// brought (or `package` brings twice): one a context.
    DuplicateCompiler {
        /// The refused package.
        package: PackageId,
        /// The context.
        context: ContextId,
        /// The package that brought it first.
        first: PackageId,
    },
    /// `package` brings a runner for `context`, which `first` already
    /// brought (or `package` brings twice): one a context.
    DuplicateRunner {
        /// The refused package.
        package: PackageId,
        /// The context.
        context: ContextId,
        /// The package that brought it first.
        first: PackageId,
    },
    /// `package` describes render engine `engine`, which `first` already
    /// described (or `package` describes twice).
    DuplicateEngine {
        /// The refused package.
        package: PackageId,
        /// The engine's id.
        engine: Arc<str>,
        /// The package that described it first.
        first: PackageId,
    },
    /// `package` brings a maker of render engine `engine`, which it
    /// doesn't describe.
    StrayMaker {
        /// The refused package.
        package: PackageId,
        /// The engine's id.
        engine: Arc<str>,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePackage(id) => write!(f, "package {id} is already registered"),
            Self::DuplicateId {
                package,
                what,
                id,
                first,
            } => write!(f, "{package} declares {what} {id}, which {first} declared"),
            Self::MalformedId { package, id } => {
                write!(f, "{package} names a creation {id:?}, which isn't an id")
            }
            Self::ForeignId { package, id } => {
                write!(f, "{package} declares {id}, under another vendor's name")
            }
            Self::UnknownDataType { package, by, data } => {
                write!(
                    f,
                    "{package}'s {by} names data type {data}, which isn't registered"
                )
            }
            Self::DuplicateHeldEdit {
                package,
                delta,
                first,
            } => write!(
                f,
                "{package} declares how {delta} deltas apply, which {first} declares"
            ),
            Self::SpareName {
                package,
                node,
                parameter,
            } => write!(
                f,
                "{package}'s node type {node} declares parameter {parameter}, a name spares take"
            ),
            Self::DuplicateConversion {
                package,
                from,
                to,
                first,
            } => write!(
                f,
                "{package} converts {from} to {to}, which {first} already does"
            ),
            Self::StrayRunner { package, context } => write!(
                f,
                "{package} brings a runner for {context}, which no package declares as a compiled context"
            ),
            Self::StrayCompiler { package, context } => write!(
                f,
                "{package} brings a compiler for {context}, which no package declares as a compiled context"
            ),
            Self::UnknownContext {
                package,
                node,
                context,
            } => write!(
                f,
                "{package}'s {node} holds a {context} network, which no package declares"
            ),
            Self::InsideCycle {
                package,
                node,
                path,
            } => {
                let path: Vec<&str> = path.iter().map(|c| c.as_str()).collect();
                write!(
                    f,
                    "{package}'s {node} would make contexts hold each other: {}",
                    path.join(" → ")
                )
            }
            Self::DuplicateCompiler {
                package,
                context,
                first,
            } => write!(
                f,
                "{package} brings a compiler for {context}, which {first} already brought"
            ),
            Self::DuplicateRunner {
                package,
                context,
                first,
            } => write!(
                f,
                "{package} brings a runner for {context}, which {first} already brought"
            ),
            Self::DuplicateEngine {
                package,
                engine,
                first,
            } => write!(
                f,
                "{package} describes render engine {engine}, which {first} described"
            ),
            Self::StrayMaker { package, engine } => write!(
                f,
                "{package} makes render engine {engine}, which it doesn't describe"
            ),
        }
    }
}

impl std::error::Error for RegistryError {}
