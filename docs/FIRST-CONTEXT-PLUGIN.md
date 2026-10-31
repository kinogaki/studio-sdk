# Write your first context plugin

A context is a kind of editing: what Tab, the keys, the menus and the inspector offer while the user works inside an object, and the selection levels they work at. This walkthrough builds a plugin with a context of its own: a Pin Board object that opens into a pinning context with two selection levels, a Pin and a Clear command, an inspector row and an entry in Create. Its tests run the UI on the SDK's fake host, as Studio runs it.

[`examples/pin-board`](../examples/pin-board) is the finished plugin, under the `example` vendor. Here it is `acme`'s. Read [Write your first node plugin](FIRST-NODE-PLUGIN.md) first: this walkthrough reuses its crate setup, entry point, kit build and install, and only says where they differ.

## 1. The crate

Set the crate up as in the node walkthrough (step 1), named `acme-pins`, with the UI layer as a normal dependency, since the UI ships in the plugin:

```toml
[package]
name = "acme-pins"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["dylib"]

[dependencies]
kinogaki-studio-api = { git = "https://github.com/kinogaki/studio-sdk" }
kinogaki-studio-ui-api = { git = "https://github.com/kinogaki/studio-sdk" }
```

The UI layer, `kinogaki-studio-ui-api` (library `studio_ui_api`), is how a plugin's UI asks Studio to draw. Plugins never see Studio's UI library: they declare data (menus, rows, levels) and make immediate-mode calls, and Studio draws. Every call runs on the UI thread and returns at once; long work goes to workers, which wake the host. No timers.

## 2. Ids and session state

The context, the object kind and the commands each have an id under the package's vendor. A command's stored name is what menus, shelves and keys hold.

The pins are **session state**: a `Slot` the package registers. Studio keeps every slot's value, undoes and redoes it with the document, and lends it to the package's UI through its call context. Session state is never saved in the project.

```rust
//! Acme's Pins plugin: a Pin Board object opening into a context with its
//! own levels, commands, inspector row and Create entry.
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

/// The context Pin Boards open into.
pub fn pinning() -> ContextId {
    ContextId::declared("acme.pins.pinning")
}

/// The Pin Board's object kind.
pub fn board() -> ObjectKindId {
    ObjectKindId::declared("acme.pins.board")
}

/// The Pin command's stored name.
pub const PIN: &str = "acme.pins.pin";
/// The Clear command's stored name.
pub const CLEAR: &str = "acme.pins.clear";

/// The pins on the board: undoable session state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pins(pub u32);

impl Slot for Pins {
    fn id() -> SlotId {
        SlotId::declared("acme.pins.pins")
    }
    fn initial() -> Self {
        Self::default()
    }
}
```

## 3. The object kind

A context is entered by opening an object whose kind names it. The Pin Board kind is placed in the scene and opens into the pinning context. Its body, the kind's own state, holds nothing here, and saves nothing but its kind.

```rust
/// A Pin Board's body.
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
    fn read(&self, _element: &dyn ElementReader, _cx: &ReadCx<'_>) -> Result<Arc<dyn ObjectBody>, Error> {
        Ok(Arc::new(BoardBody))
    }
}
```

## 4. The context's two halves

The pinning context is one `ContextId` with two halves, both the package's:

- **Its networks**, declared as data with a `NetworkContext` (step 7 registers it): every Pin Board holds one network of the context, which Studio keeps, cooks, undoes and saves. Node types offered in the context (`Description::context`) are added to it; this plugin offers none, so its networks stay empty.
- **Its UI**, a `ContextPart` of the package's `UiPackage`: what follows.

A context with only the first half is served by Studio's own UI, as the Boxes context of the node walkthrough is: the network pane, Tab and the menus listing the node types offered in it, and the node inspector. A `ContextPart` is the context's id, its `Library` and any optional parts. It adds to Studio's UI or replaces it: the library's items come first and what it sets wins, its vocabulary runs its own items, and an inspector replaces the node sheet. Studio's UI stays for what the part leaves out, and a part leaves out nothing it has to supply. The smallest is `ContextPart::new(id)`: an empty library, no levels, no commands of its own.

The UI's state lives with the host. `UiPackage::state` makes it once; for each call Studio takes it out, lends it to the part with the call's context, and puts it back. Host methods never call back into the package, so a package never meets its state missing.

```rust
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
```

## 5. The library and the vocabulary

**The library** is what the context offers Studio's menus, as data read once when the package registers: its items (each command, tool or node by its stored name, with its label, family, search words, Tab section and the surfaces that offer it), its keys, its marking menus and shelves, its right-click menu and menu bar entries, how Tab lists its items, and the viewport chrome it takes. Everything has a default; here, two commands in Tab and on right-click.

```rust
/// Pin and Clear: in Tab, and on right-click.
fn library() -> Library {
    let item = |name: &str, label: &str| VocabularyItem {
        name: name.into(),
        label: label.into(),
        family: "Pins".into(),
        section: "Pins".into(),
        places: Places { nodes: true, menus: true, tools: false, repeatable: false, objects: false },
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
        tab: TabSpec { titles: ["Pins".into(), "Pins".into()], ..TabSpec::default() },
        ..Library::default()
    }
}
```

**The vocabulary** runs the items, and says which apply now. An entry that doesn't apply is hidden, never greyed: Clear shows only once there are pins, labelled with how many it clears. Setting a slot through the context (`set_slot`) is one undo step named by its label.

```rust
/// Runs Pin and Clear.
struct PinVocabulary;

impl Vocabulary<Pinning> for PinVocabulary {
    fn name(&self) -> &str {
        "acme.pins"
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
```

Some of Studio's own menu entries run in whichever context is open, and the context answers them. **Edit ▸ Select All** is one: a context answers it by holding a library item named `command:select_all`, which is core's id for Select All. Studio then runs that item through the context's vocabulary (`Invoked::Views`). A context that selects things lists the item (here also offered in Settings for the marking menus, shelves and shortcuts) and selects everything at its active level when `run` gets it:

```rust
VocabularyItem {
    name: "command:select_all".into(),
    label: "Select All".into(),
    places: Places { menus: true, ..Places::default() },
    ..VocabularyItem::default()
}
```

Without that item Edit ▸ Select All is unavailable in the context. The pinning context selects nothing, so it doesn't list it.

## 6. Levels, the inspector and Create

**Levels** are what the user selects at (a mesh's points, edges and faces). Number keys choose them in order; the context keeps which is active.

```rust
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
```

**The inspector** shows rows through Studio's own widgets: the context builds a sheet (`sheet::Schema`) and Studio draws it. Raw widgets on `Ui` are for custom panels only.

**Create** lists object kinds a context offers at an object level; a new Pin Board opens, ready for pinning. A creation may also offer starts, a new object already holding a node; this one offers none.

```rust
/// The inspector's one row: how many pins the board holds.
struct PinInspector;

impl Inspector<Pinning> for PinInspector {
    fn show(&self, _s: &mut Pinning, cx: &mut dyn InspectorCx, ui: &mut dyn Ui) {
        let mut rows = Rows::new();
        rows.push(Row::info("pins", "Pins", cx.slot::<Pins>().0.to_string()));
        ui.sheet_keyed(&SheetStyle::new(Id::new("acme.pins")), &mut rows, None);
    }
}

/// The Pin Board in Create.
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
    fn start(&self, _s: &mut Pinning, _cx: &mut dyn Cx, _kind: ObjectKindId, start: &str) -> Result<(), String> {
        Err(format!("a Pin Board has no start {start}"))
    }
}
```

## 7. One package

The UI is part of the package it belongs to: `with_ui` adds it, and the plugin exports the one package. Its id, label and slots are the `Package`'s. Studio's registry checks the UI's contexts like every other id: under the package's vendor, and given parts by no other package.

```rust
/// The package: the context's networks, the Pin Board kind, the pins
/// slot and the context's UI.
pub fn package() -> Package {
    Package::new(PackageId::declared("acme.pins"), "Pins")
        .with_context(NetworkContext::new(pinning(), "Pins", "pins"))
        .with_object_kind(BoardKind)
        .with_slot::<Pins>()
        .with_ui(PinsUi)
}

export_package!(package);

#[cfg(test)]
mod tests;
```

## 8. Tests on the fake host

`testing::FakeHost` stands in for Studio's host interface: slots and objects' networks with a real undo history, selections, the place, views and settings as plain fields, and every request recorded. `testing::Hosting` holds the package's UI state and calls its parts as Studio does, taking the state out for each call. `testing::RecordingUi` records what a part draws, with no window.

Create `src/tests.rs`:

```rust
//! The UI on the fake host, called as Studio calls it.
use super::*;
use studio_ui_api::host::HostUndo;
use studio_ui_api::package::Registered;
use studio_ui_api::testing::{Call, FakeHost, Hosting, RecordingUi};
use studio_ui_api::{Pos, Rect, Vec2};

fn host() -> (FakeHost, Hosting) {
    let package = package();
    let host = FakeHost::new(pinning(), package.slots.clone());
    let hosted = Hosting::new(package, &host);
    (host, hosted)
}

/// Runs `item` through the context's vocabulary.
fn run(hosted: &mut Hosting, host: &mut FakeHost, item: &str) {
    hosted.with(|p, s| {
        let vocabulary = p.contexts[0].vocabulary.as_ref().unwrap();
        vocabulary.run(s, host, item, Invoked::Views)
    });
}

#[test]
fn the_package_registers_whole() {
    let mut registry = Registry::new();
    registry.register(package()).unwrap();
    assert_eq!(registry.object_kind(board()).unwrap().context(), Some(pinning()));
    assert!(registry.network_context(pinning()).is_some());
    let package = package();
    let ui = Registered::of(&package).unwrap();
    assert_eq!(ui.contexts[0].id, pinning());
}

#[test]
fn commands_are_undo_steps_and_label_themselves() {
    let (mut host, mut hosted) = host();
    run(&mut hosted, &mut host, PIN);
    run(&mut hosted, &mut host, PIN);
    assert_eq!(host.slot::<Pins>(), &Pins(2));
    let label = hosted.with(|p, s| {
        let vocabulary = p.contexts[0].vocabulary.as_ref().unwrap();
        vocabulary.label(s, &host, CLEAR)
    });
    assert_eq!(label.as_deref(), Some("Clear 2 Pins"));
    run(&mut hosted, &mut host, CLEAR);
    assert_eq!(host.history(), ["Pin", "Pin", "Clear Pins"]);
    host.undo();
    assert_eq!(host.slot::<Pins>(), &Pins(2));
}

#[test]
fn the_inspector_shows_the_count() {
    let (mut host, mut hosted) = host();
    run(&mut hosted, &mut host, PIN);
    let mut ui = RecordingUi::new(Rect::from_min_size(Pos::ZERO, Vec2::new(300.0, 200.0)));
    hosted.with(|p, s| {
        let inspector = p.contexts[0].inspector.as_ref().unwrap();
        inspector.show(s, &mut host, &mut ui)
    });
    assert_eq!(ui.calls(), [Call::Sheet(vec!["Pins".into()])]);
}
```

```sh
cargo test
```

A tool that edits an object's network goes through `HostNetwork::edit_network`. On the fake host, put a `testing::FakeNetwork` (made from the packages whose node types it holds) into `host.networks` for the object first; each edit is then one undo step, refused whole when it fails, as in Studio. The node walkthrough's last test does this.

## 9. Build and install

Build against the kit as in the node walkthrough (step 7), adding the UI layer's metadata:

```sh
cat > target/root.rs <<'EOF'
extern crate kinogaki_shared;
#[path = "../src/lib.rs"]
mod plugin;
pub use plugin::*;
EOF
rustc --edition 2024 --crate-type dylib --crate-name pins -C prefer-dynamic \
    --sysroot "$KIT/sysroot" \
    --extern kinogaki_shared="$KIT/deps/libkinogaki_shared.dylib" \
    --extern kinogaki_studio_api="$(ls "$KIT"/deps/libkinogaki_studio_api-*.rmeta)" \
    --extern studio_ui_api="$(ls "$KIT"/deps/libstudio_ui_api-*.rmeta)" \
    -L dependency="$KIT/deps" \
    target/root.rs -o target/pins.dylib
```

Install it as a node plugin, `nodes/pins.dylib` with `nodes/pins.prisma` naming `acme.pins` (node walkthrough, step 8). In Studio, a Pin Board opens into the pinning context: 1 and 2 choose Pin and Board, Tab and right-click offer Pin (and Clear once there are pins), each an undo step, and the inspector shows the count.

## Where to go next

The context parts this plugin leaves out, all in `studio_ui_api::package` with their rules in its rustdoc and [The host interface](ARCHITECTURE.md#the-host-interface):

- `ViewportLayer`: the context's tools over the views (pointer and keys, overlays through the `Painter`, the edited surface core draws, heads-up `ValueFields`);
- `ObjectRows` and `OutlineRows`: rows for the context's objects in the inspector and the outliner;
- `Results`: how the context's gestures take the host's cooks of its objects;
- a `KeyStore`, marking menus and shelves (`MenuDefaults`) in the library, and settings pages (`SettingsSection`).
