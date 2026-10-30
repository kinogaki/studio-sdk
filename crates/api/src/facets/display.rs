//! [`Displayable`]: how a value is drawn in the viewport.
use crate::cancel::CancelToken;
use crate::components::Components;
use crate::data::Data;
use crate::error::Error;
use crate::math::Bounds;
use crate::object::ObjectBody;
use crate::refs::NodeRef;
use crate::viewport::{ElementKind, ElementSet, SurfaceSource};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// How finely to prepare a value for display. Types whose display is
/// approximate (a CAD body's tessellation, a subdivision preview) pick a
/// tolerance from it; exact types ignore it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DisplayQuality {
    /// While the user drags or plays: fast, coarse.
    Interactive,
    /// The resting display.
    #[default]
    Normal,
    /// Close inspection and captures.
    Fine,
}

/// What [`Displayable::prepare`] is asked for: the quality, and the
/// object's body when the value is an object's result (its display
/// settings: a mesh's subdivision display).
#[derive(Clone, Default)]
pub struct DisplayRequest {
    /// How finely.
    pub quality: DisplayQuality,
    /// The object's body, when the value is an object's result.
    pub body: Option<Arc<dyn ObjectBody>>,
}

impl fmt::Debug for DisplayRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DisplayRequest")
            .field("quality", &self.quality)
            .field("body", &self.body.is_some())
            .finish()
    }
}

/// What a piece is to its object, which says how it looks.
#[derive(Clone)]
pub enum Role {
    /// Its own surface, styled as the object is (shaded, a ghost, a
    /// template's tint) and outlined when the object is selected or
    /// hovered.
    Own {
        /// Its polygon edges draw (a smooth surface's don't: its cage's do).
        edges: bool,
        /// Drawn as wires whatever the object's display says (a cage beside
        /// its smooth surface).
        wires: bool,
        /// Outlined with its object (a cage beside its smooth surface isn't).
        marks: bool,
    },
    /// A templated node's result: faint wires in its place, marked by the
    /// open context (the branch a live tool would take, an operand it
    /// chose). `key` is the type's name for it (a CAD body's name).
    Template {
        /// The node.
        node: NodeRef,
        /// The type's name for the piece, read by the open context.
        key: Option<Arc<dyn Any + Send + Sync>>,
    },
}

impl fmt::Debug for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Own {
                edges,
                wires,
                marks,
            } => f
                .debug_struct("Own")
                .field("edges", edges)
                .field("wires", wires)
                .field("marks", marks)
                .finish(),
            Self::Template { node, key } => f
                .debug_struct("Template")
                .field("node", node)
                .field("key", &key.is_some())
                .finish(),
        }
    }
}

/// How a piece draws while its object is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhenOpen {
    /// Not at all: the open context draws it as its edited surface.
    Hidden,
    /// As it does anywhere.
    Drawn,
}

/// One packet a value draws.
#[derive(Clone)]
pub struct Piece {
    /// What draws. Its maker reads it back as its own type with
    /// [`packet_as`](crate::viewport::packet_as).
    pub packet: Arc<dyn SurfaceSource>,
    /// Where it sits in its object, column-major; `None`: once, where the
    /// object is.
    pub placements: Option<Arc<[[[f64; 4]; 4]]>>,
    /// What it is to its object.
    pub role: Role,
    /// How it draws while its object is open.
    pub when_open: WhenOpen,
}

impl Piece {
    /// `packet` as its object's own surface, drawn once where the object
    /// is, with its edges and marks, left to the open context while its
    /// object is open.
    pub fn own(packet: Arc<dyn SurfaceSource>) -> Self {
        Self {
            packet,
            placements: None,
            role: Role::Own {
                edges: true,
                wires: false,
                marks: true,
            },
            when_open: WhenOpen::Hidden,
        }
    }
}

impl fmt::Debug for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Piece")
            .field("faces", &self.packet.face_count())
            .field("placements", &self.placements.as_ref().map(|p| p.len()))
            .field("role", &self.role)
            .field("when_open", &self.when_open)
            .finish()
    }
}

/// A value prepared for the viewport: the pieces it draws, in order, and
/// whatever the type keeps beside them to answer highlights and picks (a
/// CAD surface's face numbers to topology names). Clones share both, so
/// the host keys its caches on their allocations.
#[derive(Clone)]
pub struct Drawn {
    /// What draws, in order.
    pub pieces: Arc<[Piece]>,
    detail: Option<Arc<dyn Any + Send + Sync>>,
}

impl Drawn {
    /// `pieces` with no detail beside them.
    pub fn new(pieces: Vec<Piece>) -> Self {
        Self {
            pieces: pieces.into(),
            detail: None,
        }
    }

    /// One packet, the value's own surface ([`Piece::own`]).
    pub fn one(packet: Arc<dyn SurfaceSource>) -> Self {
        Self::new(vec![Piece::own(packet)])
    }

    /// With `detail` beside the pieces, for the type's own later calls.
    pub fn with_detail<T: Any + Send + Sync>(mut self, detail: T) -> Self {
        self.detail = Some(Arc::new(detail));
        self
    }

    /// The detail, when it is a `T`.
    pub fn detail<T: Any>(&self) -> Option<&T> {
        self.detail.as_deref().and_then(|d| d.downcast_ref())
    }
}

impl fmt::Debug for Drawn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Drawn")
            .field("pieces", &self.pieces)
            .field("detail", &self.detail.is_some())
            .finish()
    }
}

/// How a data type shows in the viewport.
///
/// `prepare` and `refine` run on a worker; the others run on the UI thread
/// with pieces `prepare` made for the same value, so they must be quick
/// (proportional to the selection, not to the value).
pub trait Displayable: Send + Sync {
    /// The component levels values offer, in the order the level bar
    /// lists them (a mesh: points, edges, faces, vertices). Empty: only
    /// whole objects are selected.
    fn levels(&self) -> &[ElementKind];

    /// `data`'s pieces as `request` asks. `previous` is what the type made
    /// for the value the object showed before, when there was one: a type
    /// whose values change a little at a time (a drag's next state) builds
    /// on it, so the host patches what it uploaded instead of uploading
    /// again ([`Surface::derived`](crate::Surface::derived)). Long
    /// preparations poll `cancel`.
    fn prepare(
        &self,
        data: &Data,
        previous: Option<&Drawn>,
        request: &DisplayRequest,
        cancel: &CancelToken,
    ) -> Result<Drawn, Error>;

    /// A finer display of `drawn` once the value rests (a CAD body's
    /// tessellation at its final tolerance); `None`: `drawn` is final. The
    /// host asks after a gesture ends and swaps the answer in when the
    /// value is still shown.
    fn refine(
        &self,
        data: &Data,
        drawn: &Drawn,
        cancel: &CancelToken,
    ) -> Result<Option<Drawn>, Error> {
        let _ = (data, drawn, cancel);
        Ok(None)
    }

    /// The elements of `drawn`'s pieces that show `components` (what the
    /// viewport outlines and tints as selected).
    fn highlight(&self, data: &Data, drawn: &Drawn, components: &Components) -> ElementSet;

    /// `components` at level `to`: faces as their edges or points, points
    /// as the faces they bound wholly. `None` when the type can't express
    /// the selection at that level.
    fn convert(&self, data: &Data, components: &Components, to: ElementKind) -> Option<Components>;

    /// The status bar's counts for `data`: per level, how many components
    /// `selection` holds and how many there are (Points 4/8). The level a
    /// selection is of counts its own components; the others count those
    /// whose points it holds wholly. Empty by default. Core asks again only
    /// when the value or the selection changes.
    fn tally(&self, data: &Data, selection: Option<&Components>) -> Vec<Tally> {
        let _ = (data, selection);
        Vec::new()
    }

    /// The box around `components` of `data`, in the value's own space:
    /// the pivot's box for a component selection. `None` when it holds
    /// nothing (or the type can't say).
    fn bounds_of(&self, data: &Data, components: &Components) -> Option<Bounds> {
        let _ = (data, components);
        None
    }
}

/// One level's count for the status bar ([`Displayable::tally`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tally {
    /// What the bar calls them ("Points").
    pub label: String,
    /// The level counted.
    pub level: ElementKind,
    /// How many the selection holds.
    pub selected: usize,
    /// How many there are.
    pub total: usize,
}
