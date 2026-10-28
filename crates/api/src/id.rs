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
//! [`ContextId`], [`FormatId`], [`NodeTypeId`], [`SlotId`],
//! [`ConversionId`]), so a node type id can't be passed where a data type
//! is meant. All of them are interned in one table: each distinct text is
//! kept once for the program's life (packages are never unloaded), so an
//! id is a `Copy` pointer-sized value and reading its text never allocates.
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Mutex;

/// Whether `text` is a well-formed id: three dot-separated segments, each
/// a lowercase ASCII letter followed by lowercase letters, digits and `_`.
///
/// ```
/// use kinogaki_studio_api::id::well_formed;
/// assert!(well_formed("kinogaki.mesh.extrude"));
/// assert!(!well_formed("kinogaki.mesh"));
/// assert!(!well_formed("kinogaki.mesh.Extrude"));
/// ```
pub fn well_formed(text: &str) -> bool {
    segments_well_formed(text, 3)
}

/// Whether `text` is a well-formed package id: two segments of the form
/// [`well_formed`] describes (`kinogaki.mesh`).
pub fn package_well_formed(text: &str) -> bool {
    segments_well_formed(text, 2)
}

fn segments_well_formed(text: &str, segments: usize) -> bool {
    let mut count = 0;
    for part in text.split('.') {
        count += 1;
        let mut chars = part.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_lowercase())
            || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return false;
        }
    }
    count == segments
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
            #[doc = concat!("`text` as a ", $what, " id; `None` unless it is [`well_formed`].")]
            pub fn parse(text: &str) -> Option<Self> {
                well_formed(text).then(|| Self(intern(text)))
            }

            #[doc = concat!("A ", $what, " id a package declares in code.")]
            ///
            /// # Panics
            ///
            /// On a malformed id: a declaration is code, and a package's
            /// tests construct every id it declares.
            pub fn declared(text: &str) -> Self {
                Self::parse(text)
                    .unwrap_or_else(|| panic!(concat!("malformed ", $what, " id {:?}"), text))
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
    /// kernel package's [`DataType`](crate::DataType).
    DataTypeId,
    "data type"
);
declare_id!(
    /// A kind of scene object (`kinogaki.core.part`,
    /// `kinogaki.mesh.object`). Registered by an
    /// [`ObjectKindSpec`](crate::ObjectKindSpec).
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
    /// [`FileFormat`](crate::FileFormat).
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
    /// by a [`SessionSlot`](crate::SessionSlot).
    SlotId,
    "session slot"
);
declare_id!(
    /// A conversion between two data types (`kinogaki.cad.tessellate`).
    /// Registered by a [`Conversion`](crate::Conversion).
    ConversionId,
    "conversion"
);

/// A package: `<vendor>.<package>` (`kinogaki.mesh`, `acme.trees`).
/// Interned like the other ids.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageId(&'static str);

impl PackageId {
    /// `text` as a package id; `None` unless it is
    /// [`package_well_formed`].
    pub fn parse(text: &str) -> Option<Self> {
        package_well_formed(text).then(|| Self(intern(text)))
    }

    /// A package id declared in code.
    ///
    /// # Panics
    ///
    /// On a malformed id.
    pub fn declared(text: &str) -> Self {
        Self::parse(text).unwrap_or_else(|| panic!("malformed package id {text:?}"))
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
    fn equal_texts_share_one_interned_string() {
        let a = DataTypeId::declared("kinogaki.test.shared");
        let b = DataTypeId::parse(&String::from("kinogaki.test.shared")).unwrap();
        assert_eq!(a, b);
        assert!(std::ptr::eq(a.as_str(), b.as_str()));
        // Every id kind shares the one table.
        let node = NodeTypeId::declared("kinogaki.test.shared");
        assert!(std::ptr::eq(a.as_str(), node.as_str()));
    }

    #[test]
    #[should_panic(expected = "malformed format id")]
    fn a_malformed_declaration_panics() {
        FormatId::declared("obj");
    }
}
