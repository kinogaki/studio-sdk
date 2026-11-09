//! Where an offering is listed: its [`Home`] at each place it is offered.
//!
//! A place is an object level (the scene root, a Part) or a network of one
//! context. Each place has **topics**: shared top-level submenus its owner
//! declares ([`Topic`]): core for the object levels and its own contexts
//! ([`topics`]), a context's package for its networks
//! ([`NetworkContext::topics`](crate::NetworkContext::topics)). Every
//! offering (a node type, a node or object creation, a library's item)
//! names one home there: one of the place's topics, or its package's
//! **family**, a top-level submenu named for the package (Spectre,
//! MaterialX); then, inside either, nested groups (plain submenus).
//!
//! The host builds every menu of a place from these homes (Tab, the menu
//! bar, the right-click menu, the marking menus' lists, Settings' pickers
//! and Tab's search), so an entry has the same path on every surface. Its
//! tops are the place's topics in the order they are declared, then the
//! families by label; inside a topic core's entries come first, then each
//! package's, packages by label, each in the order it declares them; a
//! group sits where its first entry is, and two packages naming one group
//! share it. Below the tops a group of one entry shows that entry in its
//! place, and a group alone in its menu opens in place. No list holds a
//! heading.
//!
//! ```
//! use kinogaki_studio_api::menu::{Home, topics};
//! let light = Home::topic(topics::objects::RENDER).group("Lights");
//! assert_eq!((light.topic, &light.groups[..]), (Some(topics::objects::RENDER), &["Lights".to_owned()][..]));
//! assert_eq!(Home::family().topic, None);
//! ```
use crate::id::ContextId;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Mutex;

/// A topic's id: one lowercase word, unique among its place's topics
/// (`render`). Packages name another context's topics by it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TopicId(&'static str);

/// Whether `text` is one word: a lowercase ASCII letter, then lowercase
/// letters, digits and `_`.
const fn word(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut i = 1;
    while i < bytes.len() {
        let b = bytes[i];
        if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
            return false;
        }
        i += 1;
    }
    true
}

impl TopicId {
    /// Topic `text`, declared in code (a `const`).
    ///
    /// # Panics
    ///
    /// Unless `text` is one lowercase word (checked as a `const` compiles).
    pub const fn declared(text: &'static str) -> Self {
        assert!(word(text), "malformed topic id");
        Self(text)
    }

    /// `text` as a topic id (one a host keeps in a file), interned once a
    /// process; `None` unless it is one lowercase word.
    pub fn parse(text: &str) -> Option<Self> {
        static TEXTS: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
        if !word(text) {
            return None;
        }
        // Insertion is the only write: a poisoned set is still whole.
        let mut texts = TEXTS.lock().unwrap_or_else(|e| e.into_inner());
        let kept = match texts.get(text) {
            Some(&kept) => kept,
            None => {
                let kept: &'static str = Box::leak(text.to_owned().into_boxed_str());
                texts.insert(kept);
                kept
            }
        };
        Some(Self(kept))
    }

    /// Its text (`render`).
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Debug for TopicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl fmt::Display for TopicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// A shared top-level submenu of a place ("Render"): its owner declares
/// it, and any package may list in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topic {
    /// Its id, unique within the place.
    pub id: TopicId,
    /// What menus call it.
    pub label: String,
}

impl Topic {
    /// Topic `id` called `label`.
    pub fn new(id: TopicId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
        }
    }
}

/// Where an offering is listed at a place: under one of the place's
/// topics, or in its package's family; then in nested groups.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Home {
    /// `Some`: a topic of the place; `None`: the declaring package's family.
    pub topic: Option<TopicId>,
    /// The groups below it, outermost first ("Texture 2D"); empty: directly
    /// in it.
    pub groups: Vec<String>,
}

impl Home {
    /// Directly in topic `id` of the place.
    pub fn topic(id: TopicId) -> Self {
        Self {
            topic: Some(id),
            groups: Vec::new(),
        }
    }

    /// Directly in the declaring package's family (core's package has none).
    pub fn family() -> Self {
        Self::default()
    }

    /// In group `name` inside where it is.
    pub fn group(mut self, name: impl Into<String>) -> Self {
        self.groups.push(name.into());
        self
    }
}

/// Where one item of a context's library (`studio-ui-api`'s
/// `VocabularyItem`) is listed, as a package's UI carries it for the
/// [`Registry`](crate::Registry) to check
/// ([`PackageUi::new`](crate::PackageUi::new)).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemHome {
    /// The context whose library holds it.
    pub context: ContextId,
    /// Its stored name (a node type's id for a node's item).
    pub item: String,
    /// Its own home; `None`: the node type it names gives it.
    pub home: Option<Home>,
    /// The object levels offer it too.
    pub objects: bool,
}

/// Core's places' topics, in order: the public contract packages list
/// their offerings under.
pub mod topics {
    use super::{Topic, TopicId};

    /// What reads from outside: core's File and Image nodes. Every network
    /// has it, after its context's own topics unless the context places it.
    pub const IMPORT: TopicId = TopicId::declared("import");
    /// Core's loops (For Each, Loop Input), in every network that takes
    /// them ([`NetworkContext::with_loops`](crate::NetworkContext::with_loops)),
    /// after its context's own topics unless the context places it.
    pub const LOOPS: TopicId = TopicId::declared("loops");

    /// The Import topic.
    pub fn import() -> Topic {
        Topic::new(IMPORT, "Import")
    }

    /// The Loops topic.
    pub fn loops() -> Topic {
        Topic::new(LOOPS, "Loops")
    }

    /// `table` as topics.
    fn of(table: &[(TopicId, &str)]) -> Vec<Topic> {
        table
            .iter()
            .map(|&(id, label)| Topic::new(id, label))
            .collect()
    }

    /// The object levels' (the scene root's and a Part's). Hierarchy, Edit
    /// and Select hold the levels' commands: their menus show them, Tab
    /// doesn't.
    pub mod objects {
        use super::{Topic, TopicId};

        /// What a mesh or a primitive starts as (Mesh, Primitives ▸).
        pub const GEOMETRY: TopicId = TopicId::declared("geometry");
        /// What renders the scene and lights it (Render Output, Camera, lights, Fog).
        pub const RENDER: TopicId = TopicId::declared("render");
        /// What holds materials (Material Library).
        pub const MATERIALS: TopicId = TopicId::declared("materials");
        /// The rest (Part).
        pub const UTILITY: TopicId = TopicId::declared("utility");
        /// Group, Ungroup, Parent, Unparent.
        pub const HIERARCHY: TopicId = TopicId::declared("hierarchy");
        /// Duplicate, Delete.
        pub const EDIT: TopicId = TopicId::declared("edit");
        /// Select All, Invert Selection, Select None.
        pub const SELECT: TopicId = TopicId::declared("select");

        /// Every one, in order.
        pub fn all() -> Vec<Topic> {
            super::of(&[
                (GEOMETRY, "Geometry"),
                (RENDER, "Render"),
                (MATERIALS, "Materials"),
                (UTILITY, "Utility"),
                (HIERARCHY, "Hierarchy"),
                (EDIT, "Edit"),
                (SELECT, "Select"),
            ])
        }
    }

    /// Comp's, inside a Render Output (then Import).
    pub mod comp {
        use super::{Topic, TopicId};

        /// What makes an image (Render Out).
        pub const CREATE: TopicId = TopicId::declared("create");
        /// What changes an image's colors (Levels, Grade).
        pub const COLOR: TopicId = TopicId::declared("color");
        /// What filters an image (Blur, Bloom).
        pub const FILTER: TopicId = TopicId::declared("filter");
        /// What joins images.
        pub const MERGE: TopicId = TopicId::declared("merge");
        /// What writes an image out (File Out).
        pub const OUTPUT: TopicId = TopicId::declared("output");

        /// Every one, in order.
        pub fn all() -> Vec<Topic> {
            super::of(&[
                (CREATE, "Create"),
                (COLOR, "Color"),
                (FILTER, "Filter"),
                (MERGE, "Merge"),
                (OUTPUT, "Output"),
            ])
        }
    }

    /// A Material Library's (then Import). Inside a Material (Shading)
    /// there is Import alone: everything else there is a family's.
    pub mod materials {
        use super::{Topic, TopicId};

        /// Core's Material.
        pub const MATERIALS: TopicId = TopicId::declared("materials");

        /// Every one, in order.
        pub fn all() -> Vec<Topic> {
            super::of(&[(MATERIALS, "Materials")])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_topic_id_is_one_lowercase_word() {
        for good in ["render", "texture_2d", "a1"] {
            assert!(word(good), "{good}");
        }
        for bad in ["", "Render", "1a", "two words", "kinogaki.render", "_x"] {
            assert!(!word(bad), "{bad}");
        }
        assert_eq!(topics::comp::FILTER.as_str(), "filter");
        assert_eq!(TopicId::parse("filter"), Some(topics::comp::FILTER));
        assert_eq!(TopicId::parse("Filter"), None);
    }

    #[test]
    fn core_places_topics_are_unique_and_in_order() {
        let labels = |t: Vec<Topic>| t.into_iter().map(|t| t.label).collect::<Vec<_>>();
        assert_eq!(
            labels(topics::objects::all()),
            [
                "Geometry",
                "Render",
                "Materials",
                "Utility",
                "Hierarchy",
                "Edit",
                "Select"
            ]
        );
        assert_eq!(
            labels(topics::comp::all()),
            ["Create", "Color", "Filter", "Merge", "Output"]
        );
        let home = Home::family().group("Surface").group("PBR");
        assert_eq!((home.topic, home.groups.len()), (None, 2));
    }
}
