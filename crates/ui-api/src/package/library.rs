//! A context's library ([`Library`]): what it offers the host's menus,
//! keys, Tab and viewport chrome, as data. The host reads it once, when
//! the package registers, and builds every surface from it: Tab, the
//! held-key marking menus, the shelves, the right-click menu, the menu
//! bar's Create, Select, Modify and Transform, the key bindings and the
//! viewport's shared chrome. A context names its items by their stored
//! names in its [`Vocabulary`](super::Vocabulary), which runs them.
//!
//! **Entries that can't apply are hidden, never greyed.** Each entry shows
//! where its [`When`] holds (the active level, whether anything is
//! selected, whether the displayed node is a held edit), and where its
//! context's [`Vocabulary::shown`](super::Vocabulary::shown) keeps it.
//!
//! **What the user edits.** The marking menus and the shelves are the
//! user's to change in Settings; the defaults here are what they start as
//! and return to. They are kept under their [`MenuDefaults::store`] name,
//! so several contexts can share one set ([`Shared::Of`]). The right-click
//! menu and the menu bar ([`ViewMenus`]) are fixed: the context's as
//! given, never edited by the user.
use super::Popover;
use crate::Key;
use crate::gizmo::Pivot;
use crate::input::Modifiers;
use kinogaki_studio_api::ContextId;

/// What a context offers the host's menus, keys, Tab and viewport chrome.
/// Read once, when the package registers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    /// Its items, in the order Tab and Settings list them: every name a
    /// menu, shelf, key or Tab entry of the context holds.
    pub items: Vec<VocabularyItem>,
    /// Its key store; `None`: the viewport's keys alone.
    pub keys: Option<Shared<KeyStore>>,
    /// Its A/S/D marking menus and shelves; `None`: neither.
    pub menus: Option<Shared<MenuDefaults>>,
    /// Its right-click menu and menu bar entries; `None`: neither.
    pub view_menus: Option<Shared<ViewMenus>>,
    /// How Tab lists its items.
    pub tab: TabSpec,
    /// The viewport's shared chrome it takes.
    pub viewport: ViewportSpec,
}

/// A part of a library a context gives itself or shares with another
/// context of the same package (or a core one): a Material's shading
/// network keeps its library's keys, and both show the scene and take its
/// menus.
#[derive(Clone, Debug, PartialEq)]
pub enum Shared<T> {
    /// The context's own; its entries name its own items.
    Own(T),
    /// Context `ContextId`'s (a network's, which gives it as `Own`); its
    /// entries name that context's items, and run there.
    Of(ContextId),
}

/// One item of a context's vocabulary: a tool, command or node a menu,
/// shelf, key or Tab entry holds, by its stored name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VocabularyItem {
    /// Its stored name ("acme.trees.branch", "command:prune",
    /// "node:acme.trees.branch"), unique in its context.
    pub name: String,
    /// What menus call it.
    pub label: String,
    /// The family menus group it under (a submenu, a Tab subhead).
    pub family: String,
    /// Lower-case synonyms for search.
    pub aliases: Vec<String>,
    /// Tab's category for it ("Polygon").
    pub section: String,
    /// Where it applies: an entry of it shows only where this holds, on
    /// every surface.
    pub when: When,
    /// Which surfaces offer it.
    pub places: Places,
}

/// The surfaces that offer an item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Places {
    /// Tab lists it where Tab adds nodes: over the network, and over the
    /// views while they don't list tools ([`TabSpec::tools`]).
    pub nodes: bool,
    /// Tab lists it over the views while they list tools.
    pub tools: bool,
    /// Settings offers it for the marking menus, shelves and shortcuts.
    pub menus: bool,
}

/// Where an entry shows: every condition holds. The default shows always.
/// The level is that of what is selected, else the primary level
/// ([`Levels::selected`](super::Levels::selected)).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct When {
    /// The levels it shows at, by their ids ([`LevelSpec::id`](super::LevelSpec::id));
    /// empty: every level (and a context without levels).
    pub levels: Vec<String>,
    /// Only with something selected (`Some(true)`), or only with nothing
    /// selected (`Some(false)`).
    pub selected: Option<bool>,
    /// Only while the displayed node is a held edit (`Some(true)`, Edit
    /// Mesh), or only outside one (`Some(false)`).
    pub editing: Option<bool>,
}

impl When {
    /// At the levels `levels` (their ids).
    pub fn levels<S: Into<String>>(levels: impl IntoIterator<Item = S>) -> Self {
        When {
            levels: levels.into_iter().map(Into::into).collect(),
            ..When::default()
        }
    }

    /// Whether it holds at level `level` (`None`: a context without
    /// levels), with something `selected` or not, `editing` or not.
    pub fn holds(&self, level: Option<&str>, selected: bool, editing: bool) -> bool {
        let level =
            self.levels.is_empty() || level.is_some_and(|l| self.levels.iter().any(|x| x == l));
        level
            && self.selected.is_none_or(|s| s == selected)
            && self.editing.is_none_or(|e| e == editing)
    }
}

/// A context's own key store: its default bindings, which apply over its
/// views and fall back along its chain (its own store, its `parent`'s
/// chain, the viewport's, Global). A package binds only in its own
/// stores; Global's and the viewport's are the host's.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyStore {
    /// Its stable name, the key the user's bindings are kept under
    /// ("Mesh"), unique among installed stores.
    pub id: String,
    /// What Settings ▸ Shortcuts calls it ("Edit Mesh").
    pub label: String,
    /// Another store it falls back to before the viewport's (a sketch's
    /// over its CAD object's): a store of the same package, or a core one.
    pub parent: Option<String>,
    /// When it is the views' store.
    pub active: KeysActive,
    /// Its default bindings.
    pub defaults: Vec<KeyBinding>,
}

/// When a context's key store is the one over its views.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeysActive {
    /// While the context is open.
    #[default]
    Always,
    /// While the displayed node is a held edit (Edit Mesh); otherwise
    /// the viewport's keys alone.
    Editing,
}

/// A default key binding of a vocabulary item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyBinding {
    /// The item, by its stored name.
    pub item: String,
    /// The key.
    pub key: Key,
    /// The modifiers held with it (shortcuts compare
    /// [`Modifiers::command`], never Control or Command themselves).
    pub modifiers: Modifiers,
}

/// A context's A/S/D marking menus and shelves, as they start: the user
/// edits them in Settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuDefaults {
    /// The name the user's edits are kept under ("Mesh"), unique among
    /// installed menus.
    pub store: String,
    /// The A, S and D menus' titles.
    pub titles: [String; 3],
    /// The menus at each level (a context without levels: its one level,
    /// "Objects"). A menu and level not listed starts empty.
    pub marking: Vec<MarkingDefault>,
    /// The shelves; empty: no shelf.
    pub shelves: Vec<Shelf>,
}

/// One held-key menu at one level, as it starts.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkingDefault {
    /// Which menu.
    pub menu: Held,
    /// The level, by its id ("Objects" for a context without levels).
    pub level: String,
    /// The eight compass positions, North first, clockwise; items by name.
    pub compass: [Option<String>; 8],
    /// The list below South.
    pub list: Vec<String>,
}

/// A held-key menu: A, S or D.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Held {
    /// Held A: what the context makes.
    A,
    /// Held S: what modifies the selection.
    S,
    /// Held D: what selects.
    D,
}

impl Held {
    /// A, S and D, in order.
    pub const ALL: [Held; 3] = [Held::A, Held::S, Held::D];
}

/// A shelf as it starts: its name and its tools and dividers in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Shelf {
    /// Its tab's name.
    pub name: String,
    /// Its tiles and dividers.
    pub entries: Vec<ShelfEntry>,
}

/// One place on a shelf.
#[derive(Clone, Debug, PartialEq)]
pub enum ShelfEntry {
    /// An item's tile.
    Tool {
        /// The item, by name.
        item: String,
        /// A shorter label than the item's; `None`: the item's.
        label: Option<String>,
    },
    /// A divider with a name above the tiles after it (empty: none).
    Divider(String),
}

/// A context's right-click menu over the views and its entries in the
/// menu bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewMenus {
    /// The right-click menus: the first whose `when` holds shows.
    pub context: Vec<ContextMenuSpec>,
    /// The menu bar's sections: each menu lists those whose `when` holds.
    pub bar: Vec<BarSection>,
}

/// One right-click menu over the views.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextMenuSpec {
    /// Where it is the menu.
    pub when: When,
    /// Its muted title ("Edit Mesh").
    pub title: String,
    /// Its entries, grouped in submenus by their items' families.
    pub entries: Vec<MenuEntry>,
    /// Its Select ▸ submenu; empty: none.
    pub select: Vec<MenuEntry>,
    /// Select ▸ lists plain entries, not family submenus.
    pub plain_select: bool,
    /// Its last button, which opens Tab ("All Tools…  Tab").
    pub browse: String,
}

/// One run of a menu bar menu's entries.
#[derive(Clone, Debug, PartialEq)]
pub struct BarSection {
    /// Which menu.
    pub menu: BarMenu,
    /// Where the menu lists it.
    pub when: When,
    /// A submenu's name ("Polygon"); `None`: its families in place.
    pub label: Option<String>,
    /// Its entries, grouped in submenus by their items' families.
    pub entries: Vec<MenuEntry>,
}

/// The menu bar's menus a context fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarMenu {
    /// Create.
    Create,
    /// Select.
    Select,
    /// Modify.
    Modify,
    /// Transform.
    Transform,
}

/// An entry of a right-click or menu bar menu.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntry {
    /// An item, by name.
    Item(String),
    /// Repeat: the item that repeats the last tool, by name. It leads the
    /// menu, set off by a separator, and shows while
    /// [`Vocabulary::shown`](super::Vocabulary::shown) keeps it.
    Repeat(String),
}

/// How Tab lists a context's items.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabSpec {
    /// Its titles: listing tools ("Edit Mesh tools"), and listing nodes
    /// ("Geo nodes").
    pub titles: [String; 2],
    /// When Tab over the views lists tools ([`Places::tools`]) rather
    /// than nodes; `None`: never.
    pub tools: Option<When>,
    /// A first category of the tools that apply now, while Tab lists
    /// tools.
    pub suggested: Option<Suggested>,
    /// Per level, items that lead their section there (Mesh's Loops at
    /// the Edge level, its Region elsewhere).
    pub leading: Vec<LevelItems>,
}

/// Tab's leading category of the tools that apply now (CAD's Suggested).
#[derive(Clone, Debug, PartialEq)]
pub struct Suggested {
    /// Its name.
    pub title: String,
    /// Where it shows.
    pub when: When,
    /// The sections it takes tools from; empty: every section.
    pub sections: Vec<String>,
    /// The tools leading it at each level (a level's usual tools first).
    pub first: Vec<LevelItems>,
}

/// Items of one level.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelItems {
    /// The level, by its id.
    pub level: String,
    /// The items, by name.
    pub items: Vec<String>,
}

/// The viewport chrome a context takes from the host.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewportSpec {
    /// The Shading popover's shared rows it offers; one it leaves out
    /// shows at its default.
    pub shared_rows: SharedRows,
    /// Analysis modes it adds after the shared ones under Shaded (CAD's
    /// Deviation): each draws the edited surface's scalars
    /// ([`Edited::scalars`](super::Edited::scalars)).
    pub analysis_modes: Vec<AnalysisMode>,
    /// Whether the Snap popover shows the shared steps (Snap from, Rotate
    /// and Scale step): the moves here use them.
    pub snap_steps: bool,
    /// The Snap master here (the Snap button's switch, X): `None`, the
    /// one the contexts without their own share; `Some(on)`, the
    /// context's own (each mode's apart), starting at `on`. The host keeps
    /// either in the user's settings.
    pub own_snapping: Option<bool>,
    /// Where the transform pivot starts.
    pub default_pivot: Pivot,
    /// The popovers among Gear, Components and Snap whose buttons show,
    /// with the context's [rows](super::ViewportLayer::rows). Shading,
    /// Viewport and Camera always show.
    pub popovers: Vec<Popover>,
}

impl Default for ViewportSpec {
    fn default() -> Self {
        ViewportSpec {
            shared_rows: SharedRows::ALL,
            analysis_modes: Vec::new(),
            snap_steps: true,
            own_snapping: None,
            default_pivot: Pivot::Centre,
            popovers: Vec::new(),
        }
    }
}

/// The Shading and Viewport popovers' rows the host owns, each offered or
/// not. Wireframe, Solid and Render always show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedRows {
    /// Flat or Shaded (under Solid).
    pub flat_shaded: bool,
    /// The wireframe over Solid.
    pub wire_on_solid: bool,
    /// Back faces tinted.
    pub tint_back_faces: bool,
    /// Back faces culled.
    pub backface_culling: bool,
    /// The shared analysis modes (Zebra, Isophotes…) and their settings.
    pub analysis: bool,
    /// Viewport ▸ the rest of the scene, inside an object.
    pub rest_of_scene: bool,
}

impl SharedRows {
    /// Every row.
    pub const ALL: SharedRows = SharedRows {
        flat_shaded: true,
        wire_on_solid: true,
        tint_back_faces: true,
        backface_culling: true,
        analysis: true,
        rest_of_scene: true,
    };
    /// No row.
    pub const NONE: SharedRows = SharedRows {
        flat_shaded: false,
        wire_on_solid: false,
        tint_back_faces: false,
        backface_culling: false,
        analysis: false,
        rest_of_scene: false,
    };
}

/// An analysis mode a context adds to the Shading popover.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisMode {
    /// Its row's label ("Deviation").
    pub label: String,
    /// Its icon, by name.
    pub icon: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_holds_where_every_condition_does() {
        let any = When::default();
        assert!(any.holds(None, false, false) && any.holds(Some("Face"), true, true));
        let faces = When::levels(["Face"]);
        assert!(faces.holds(Some("Face"), false, false));
        assert!(!faces.holds(Some("Edge"), false, false));
        assert!(!faces.holds(None, false, false), "a context without levels");
        let selected = When {
            selected: Some(true),
            ..When::default()
        };
        assert!(selected.holds(None, true, false) && !selected.holds(None, false, false));
        let outside = When {
            editing: Some(false),
            ..When::default()
        };
        assert!(outside.holds(None, false, false) && !outside.holds(None, false, true));
    }
}
