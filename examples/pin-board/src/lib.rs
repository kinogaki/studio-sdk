//! A Kinogaki Studio plugin that brings a context with its UI.
//!
//! The package adds an object kind, the Pin Board
//! (`example.pins.board`), which opens into the package's context,
//! `example.pins.pinning`. A context is one id with two halves, both
//! declared here: its networks, as data
//! ([`NetworkContext`], [`Package::with_context`]: every Pin Board holds
//! one, which Studio keeps), and its UI, added with [`WithUi::with_ui`]:
//!
//! - the Pin Board in an object level's Create menu;
//! - two selection levels, Pin and Board (number keys 1 and 2 choose
//!   them);
//! - a Pin command and a Clear command, in Tab and on right-click (Clear
//!   only once there are pins, labelled with how many it clears);
//! - an inspector row saying how many pins the board holds.
//!
//! The pins are a [session slot](kinogaki_studio_api::Slot) of the
//! package's, so Studio undoes them with the document. Everything,
//! headless parts and UI, is one [`Package`], exported once.
use kinogaki_studio_api::element::{ElementReader, ElementWriter};
use kinogaki_studio_api::*;
use std::sync::Arc;
use studio_ui_api::host::{Cx, HostSlotsExt, InspectorCx, Waker};
use studio_ui_api::package::{
    ContextMenuSpec, ContextPart, Creation, Creations, Inspector, Invoked, LevelSpec, Levels,
    Library, MenuEntry, Places, Shared, TabSpec, UiPackage, ViewMenus, Vocabulary, VocabularyItem,
    WithUi,
};
use studio_ui_api::sheet::{Field as Row, Schema as Rows};
use studio_ui_api::{Id, SheetStyle, Ui, UiExt};

/// The package's id.
pub fn id() -> PackageId {
    PackageId::declared("example.pins")
}

/// The context Pin Boards open into.
pub fn pinning() -> ContextId {
    ContextId::declared("example.pins.pinning")
}

/// The Pin Board's object kind.
pub fn board() -> ObjectKindId {
    ObjectKindId::declared("example.pins.board")
}

/// The Pin command's item name.
pub const PIN: &str = "example.pins.pin";
/// The Clear command's item name.
pub const CLEAR: &str = "example.pins.clear";

/// The pins on the board: undoable session state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pins(pub u32);

impl Slot for Pins {
    fn id() -> SlotId {
        SlotId::declared("example.pins.pins")
    }
    fn initial() -> Self {
        Self::default()
    }
}

/// A Pin Board's body: it holds nothing of its own (its pins are the
/// session's).
#[derive(Debug, Default)]
pub struct BoardBody;

impl ObjectBody for BoardBody {
    fn kind(&self) -> ObjectKindId {
        board()
    }
}

/// Pin Boards: placed objects opening into the pinning context.
struct BoardKind;

impl ObjectKindSpec for BoardKind {
    fn id(&self) -> ObjectKindId {
        board()
    }
    fn label(&self) -> &str {
        "Pin Board"
    }
    fn icon(&self) -> &str {
        "cube"
    }
    fn placed(&self) -> bool {
        true
    }
    fn context(&self) -> Option<ContextId> {
        Some(pinning())
    }
    fn result(&self) -> Option<DataTypeId> {
        None
    }
    fn new_body(&self) -> Arc<dyn ObjectBody> {
        Arc::new(BoardBody)
    }
    fn codec(&self) -> &dyn ObjectCodec {
        &BoardCodec
    }
    fn adopts(&self, _data: &Data) -> Option<Arc<dyn ObjectBody>> {
        None
    }
}

/// A board saves nothing but its kind.
struct BoardCodec;

impl ObjectCodec for BoardCodec {
    fn write(&self, _body: &dyn ObjectBody, _out: &mut dyn ElementWriter) -> Result<(), Error> {
        Ok(())
    }
    fn read(
        &self,
        _element: &dyn ElementReader,
        _cx: &ReadCx<'_>,
    ) -> Result<Arc<dyn ObjectBody>, Error> {
        Ok(Arc::new(BoardBody))
    }
}

/// The UI's state: the active level.
#[derive(Debug, Default)]
pub struct Pinning {
    /// The active level: 0 Pin, 1 Board.
    pub level: usize,
}

/// The pinning context's UI.
struct PinsUi;

impl UiPackage for PinsUi {
    type State = Pinning;
    fn state(&self, _wake: Waker) -> Pinning {
        Pinning::default()
    }
    fn contexts(&self) -> Vec<ContextPart<Pinning>> {
        vec![
            ContextPart::new(pinning())
                .with_library(library())
                .with_levels(PinLevels)
                .with_vocabulary(PinVocabulary)
                .with_inspector(PinInspector)
                .with_creations(BoardCreation),
        ]
    }
}

/// Pin and Clear: in Tab, and on right-click.
fn library() -> Library {
    let item = |name: &str, label: &str| VocabularyItem {
        name: name.into(),
        label: label.into(),
        family: "Pins".into(),
        section: "Pins".into(),
        places: Places {
            nodes: true,
            menus: true,
            tools: false,
            repeatable: false,
        },
        ..VocabularyItem::default()
    };
    Library {
        items: vec![item(PIN, "Pin"), item(CLEAR, "Clear")],
        view_menus: Some(Shared::Own(ViewMenus {
            context: vec![ContextMenuSpec {
                when: Default::default(),
                title: "Pins".into(),
                entries: vec![MenuEntry::Item(PIN.into()), MenuEntry::Item(CLEAR.into())],
                select: Vec::new(),
                plain_select: false,
                browse: "Add…  Tab".into(),
            }],
            bar: Vec::new(),
        })),
        tab: TabSpec {
            titles: ["Pins".into(), "Pins".into()],
            ..TabSpec::default()
        },
        ..Library::default()
    }
}

/// Pin and Board: one active at a time.
struct PinLevels;

impl Levels<Pinning> for PinLevels {
    fn levels(&self) -> Vec<LevelSpec> {
        [("Pin", ElementKind::Point), ("Board", ElementKind::Object)]
            .map(|(name, level)| LevelSpec {
                id: name.into(),
                label: name.into(),
                icon: "cube".into(),
                level,
                combines: false,
            })
            .into()
    }
    fn active(&self, s: &Pinning, _cx: &dyn Cx) -> Vec<usize> {
        vec![s.level]
    }
    fn primary(&self, s: &Pinning, _cx: &dyn Cx) -> usize {
        s.level
    }
    fn choose(&self, s: &mut Pinning, _cx: &mut dyn Cx, level: usize) {
        s.level = level;
    }
    fn toggle(&self, s: &mut Pinning, cx: &mut dyn Cx, level: usize) {
        self.choose(s, cx, level);
    }
}

/// Runs Pin and Clear; Clear shows once there are pins, saying how many.
struct PinVocabulary;

impl Vocabulary<Pinning> for PinVocabulary {
    fn name(&self) -> &str {
        "example.pins"
    }
    fn run(&self, _s: &mut Pinning, cx: &mut dyn Cx, item: &str, _how: Invoked) {
        let pins = cx.slot::<Pins>().0;
        match item {
            PIN => cx.set_slot("Pin", Pins(pins + 1)),
            CLEAR => cx.set_slot("Clear Pins", Pins(0)),
            _ => {}
        }
    }
    fn shown(&self, _s: &Pinning, cx: &dyn Cx, item: &str) -> bool {
        item != CLEAR || cx.slot::<Pins>().0 > 0
    }
    fn label(&self, _s: &Pinning, cx: &dyn Cx, item: &str) -> Option<String> {
        let n = cx.slot::<Pins>().0;
        (item == CLEAR).then(|| format!("Clear {n} Pins"))
    }
}

/// The Pin Board in Create: a new board opens, ready for pinning. It
/// offers no starts (a new object already holding a node).
struct BoardCreation;

impl Creations<Pinning> for BoardCreation {
    fn creations(&self) -> Vec<Creation> {
        vec![Creation {
            kind: board(),
            label: "Pin Board".into(),
            aliases: vec!["corkboard".into()],
            opens: true,
            root_only: None,
            starts: Vec::new(),
        }]
    }
    fn start(
        &self,
        _s: &mut Pinning,
        _cx: &mut dyn Cx,
        _kind: ObjectKindId,
        start: &str,
    ) -> Result<(), String> {
        Err(format!("a Pin Board has no start {start}"))
    }
}

/// The inspector's one row: how many pins the board holds.
struct PinInspector;

impl Inspector<Pinning> for PinInspector {
    fn show(&self, _s: &mut Pinning, cx: &mut dyn InspectorCx, ui: &mut dyn Ui) {
        let mut rows = Rows::new();
        rows.push(Row::info("pins", "Pins", cx.slot::<Pins>().0.to_string()));
        ui.sheet_keyed(&SheetStyle::new(Id::new("example.pins")), &mut rows, None);
    }
}

/// The package: the pinning context's networks, the Pin Board kind, the
/// pins slot and the context's UI.
pub fn package() -> Package {
    Package::new(id(), "Pins")
        .with_context(NetworkContext::new(pinning(), "Pins", "pins"))
        .with_object_kind(BoardKind)
        .with_slot::<Pins>()
        .with_ui(PinsUi)
}

export_package!(package);

#[cfg(test)]
mod tests;
