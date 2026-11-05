//! Declared ids: what packages name everything they contribute by.
//!
//! Every id is `<vendor>.<package>.<name>`: three dot-separated segments,
//! each a lowercase ASCII letter followed by lowercase letters, digits and
//! `_` (`kinogaki.mesh.extrude`, `kinogaki.cad.body`,
//! `kinogaki.format.obj`). An id is declared by the package and never
//! derived from a Rust identifier, so renaming code never changes what a
//! project file or a shelf stores. A package's own id ([`PackageId`]) is
//! the first two segments.
//!
//! Each kind of id is its own type ([`DataTypeId`], [`ObjectKindId`],
//! [`ContextId`], [`FormatId`], [`NodeTypeId`], [`SlotId`], [`StoredId`],
//! [`MesherId`], [`VerbId`], [`CreationId`], [`EngineId`]), so a node type
//! id can't be passed where a data type is meant. An id is a `Copy` value over `'static` text: one a package
//! declares in code is its literal (made in a `const`, with no lock or
//! allocation), and one parsed from a file is interned in one table (each
//! distinct text kept once for the program's life: packages are never
//! unloaded). Ids compare by their text.
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Mutex;

/// Whether `text` is a well-formed id: three dot-separated segments, each
/// a lowercase ASCII letter followed by lowercase letters, digits and `_`.
const fn well_formed(text: &str) -> bool {
    segments_well_formed(text, 3)
}

/// Whether `text` is a well-formed package id: two segments of the form
/// [`well_formed`] describes (`kinogaki.mesh`).
const fn package_well_formed(text: &str) -> bool {
    segments_well_formed(text, 2)
}

/// Whether `text` is `segments` dot-separated segments, each a lowercase
/// ASCII letter followed by lowercase letters, digits and `_` (a `const
/// fn`, so a declared id is checked where it is made).
const fn segments_well_formed(text: &str, segments: usize) -> bool {
    let bytes = text.as_bytes();
    let (mut count, mut start, mut i) = (1, true, 0);
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'.' {
            if start {
                return false;
            }
            count += 1;
            start = true;
        } else if start {
            if !b.is_ascii_lowercase() {
                return false;
            }
            start = false;
        } else if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
            return false;
        }
        i += 1;
    }
    !start && count == segments
}

/// `text` interned, when it is a well-formed id: an id another crate
/// keeps as text (a render engine's) checked by the same rule.
pub fn parse_text(text: &str) -> Option<&'static str> {
    well_formed(text).then(|| intern(text))
}

/// The one table of every id's text: each distinct text is leaked once and
/// shared by every id type that names it.
fn intern(text: &str) -> &'static str {
    static TEXTS: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
    // A panic while holding the lock can't leave the set inconsistent
    // (insertion is the only write), so a poisoned lock is still usable.
    let mut texts = TEXTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&kept) = texts.get(text) {
        return kept;
    }
    let kept: &'static str = Box::leak(text.to_owned().into_boxed_str());
    texts.insert(kept);
    kept
}

/// Declares one id type: a `Copy` newtype over interned text with
/// `parse`, `declared`, `as_str`, `package` and `name`.
macro_rules! declare_id {
    ($(#[$doc:meta])* $name:ident, $what:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(&'static str);

        impl $name {
            #[doc = concat!("`text` as a ", $what, " id; `None` unless it is `well_formed`.")]
            pub fn parse(text: &str) -> Option<Self> {
                well_formed(text).then(|| Self(intern(text)))
            }

            #[doc = concat!("A ", $what, " id a package declares in code: a `const`")]
            /// (in a `const` item it is checked as the code compiles).
            ///
            /// # Panics
            ///
            /// On a malformed id: a declaration is code, and a package's
            /// tests construct every id it declares.
            pub const fn declared(text: &'static str) -> Self {
                assert!(well_formed(text), concat!("malformed ", $what, " id"));
                Self(text)
            }

            /// The id's text (`kinogaki.mesh.extrude`).
            pub fn as_str(self) -> &'static str {
                self.0
            }

            /// The package part, the first two segments (`kinogaki.mesh`).
            pub fn package(self) -> &'static str {
                self.0.rsplit_once('.').map_or(self.0, |(package, _)| package)
            }

            /// The last segment (`extrude`).
            pub fn name(self) -> &'static str {
                self.0.rsplit_once('.').map_or(self.0, |(_, name)| name)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:?}", self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.0)
            }
        }
    };
}

declare_id!(
    /// A data type: what a wire carries and what an object's result is
    /// (`kinogaki.mesh.geometry`, `kinogaki.cad.body`). Registered by a
    /// kernel package's `DataType`.
    DataTypeId,
    "data type"
);
declare_id!(
    /// A kind of scene object (`kinogaki.core.part`,
    /// `kinogaki.mesh.object`). Registered by an
    /// `ObjectKindSpec`.
    ObjectKindId,
    "object kind"
);
declare_id!(
    /// A context: a kind of network and the views inside it
    /// (`kinogaki.mesh.network`). The headless API only names contexts;
    /// what a context does is the host UI interface's (`studio-ui-api`).
    ContextId,
    "context"
);
declare_id!(
    /// A file format (`kinogaki.format.obj`). Registered by a
    /// `FileFormat`.
    FormatId,
    "format"
);
declare_id!(
    /// A node type (`kinogaki.mesh.extrude`). Project files store a node
    /// by it, shelves and marking menus name nodes by it.
    NodeTypeId,
    "node type"
);
declare_id!(
    /// A package's session slot (`kinogaki.mesh.selection`). Registered
    /// by a `SessionSlot`.
    SlotId,
    "session slot"
);
declare_id!(
    /// A package's stored value (`kinogaki.core.tolerances`): what names
    /// it in the project and the user's settings. Registered by a
    /// `StoredValue`.
    StoredId,
    "stored value"
);
declare_id!(
    /// A B-rep mesher (`kinogaki.cad.mesher`). Registered by a
    /// `Mesher`.
    MesherId,
    "mesher"
);
declare_id!(
    /// A verb: a function a kernel offers every other package
    /// (`kinogaki.mesh_kernel.boolean`). Registered by a `Verb`.
    VerbId,
    "verb"
);
declare_id!(
    /// A creation: one way menus, keys and shelves make a node
    /// (`NodeCreation`) or an object
    /// (`KindCreation`), under its package's vendor
    /// (`kinogaki.render.spot_light`).
    CreationId,
    "creation"
);

declare_id!(
    /// A render engine (`kinogaki.spectre.engine`): what render settings
    /// and files name it by. Registered by an `EngineDescription`.
    EngineId,
    "engine"
);

/// A package: `<vendor>.<package>` (`kinogaki.mesh`, `acme.trees`).
/// Interned like the other ids.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageId(&'static str);

impl PackageId {
    /// `text` as a package id; `None` unless it is
    /// `package_well_formed`.
    pub fn parse(text: &str) -> Option<Self> {
        package_well_formed(text).then(|| Self(intern(text)))
    }

    /// A package id declared in code: a `const`.
    ///
    /// # Panics
    ///
    /// On a malformed id.
    pub const fn declared(text: &'static str) -> Self {
        assert!(package_well_formed(text), "malformed package id");
        Self(text)
    }

    /// The id's text (`kinogaki.mesh`).
    pub fn as_str(self) -> &'static str {
        self.0
    }

    /// The vendor, the first segment (`kinogaki`).
    pub fn vendor(self) -> &'static str {
        self.0.split_once('.').map_or(self.0, |(vendor, _)| vendor)
    }
}

impl fmt::Debug for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_three_lowercase_segments() {
        for good in ["kinogaki.mesh.extrude", "acme.trees.l2_system", "a.b.c"] {
            assert!(well_formed(good), "{good}");
        }
        for bad in [
            "kinogaki.mesh",
            "kinogaki.mesh.Extrude",
            "kinogaki..extrude",
            "kinogaki.mesh.extrude.2",
            "kinogaki.mesh.2d",
            "kinogaki.mesh.extrude ",
            "op:Bevel",
            "",
        ] {
            assert!(!well_formed(bad), "{bad}");
            assert!(NodeTypeId::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn package_ids_are_two_segments() {
        assert!(package_well_formed("kinogaki.mesh"));
        assert!(!package_well_formed("kinogaki"));
        assert!(!package_well_formed("kinogaki.mesh.extrude"));
        let id = PackageId::declared("acme.trees");
        assert_eq!(id.vendor(), "acme");
        assert_eq!(id.to_string(), "acme.trees");
    }

    #[test]
    fn ids_split_into_package_and_name() {
        let id = NodeTypeId::declared("kinogaki.mesh.extrude");
        assert_eq!(id.package(), "kinogaki.mesh");
        assert_eq!(id.name(), "extrude");
        assert_eq!(format!("{id:?}"), "\"kinogaki.mesh.extrude\"");
    }

    #[test]
    fn declared_and_parsed_ids_of_one_text_are_one_id() {
        const DECLARED: DataTypeId = DataTypeId::declared("kinogaki.test.shared");
        let parsed = DataTypeId::parse(&String::from("kinogaki.test.shared")).unwrap();
        assert_eq!(DECLARED, parsed);
        // Parsed texts are interned once, in one table for every id kind.
        let node = NodeTypeId::parse("kinogaki.test.shared").unwrap();
        assert!(std::ptr::eq(parsed.as_str(), node.as_str()));
        assert!(!well_formed("kinogaki..mesh") && !well_formed("kinogaki.mesh."));
    }

    #[test]
    #[should_panic(expected = "malformed format id")]
    fn a_malformed_declaration_panics() {
        FormatId::declared("obj");
    }
}
