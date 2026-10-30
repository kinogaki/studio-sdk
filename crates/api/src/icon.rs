//! [`Icon`]: the named icons a package adds to Studio's icon set.
//!
//! **Names.** Wherever the API asks for an icon by name
//! ([`ObjectKindSpec::icon`](crate::ObjectKindSpec::icon),
//! [`Description::icon`](crate::cook::Description::icon), and the UI
//! layer's buttons, painter icons and library entries), a plain name
//! (`"cube"`) is one of Studio's own icons, the default set. A package's
//! own icon is named by the package's id, a `/` and the icon's name
//! ([`qualified`]): icon `"leaf"` of package `acme.trees` is
//! `"acme.trees/leaf"`. A package registers its icons under its own id
//! alone, so two packages' names never meet.
//!
//! **What an icon is.** An SVG document of at most [`MAX_BYTES`] bytes,
//! drawn the way Studio draws its own icons: as pixel art, each icon pixel
//! a solid block. Its `viewBox` is its size in icon pixels: it starts at
//! `0 0` and is whole numbers from 1 to [`MAX_PIXELS`] wide and tall (the
//! size of Studio's own, so it fits wherever theirs do). Studio draws
//! every pixel solid or clear, with `currentColor` white, and tints it
//! with the color of the widget showing it (the ink, the accent when
//! selected, the hover look), so an icon is one color: draw it in white
//! or `currentColor`, ideally as whole-pixel `<rect>`s. Shapes, paths,
//! transforms and fills are read as a browser reads them.
//!
//! **Checks.** Studio checks each icon when the package registers: its
//! name ([`name_well_formed`]), its size and its text ([`Icon::check`]),
//! then whether its document reads and its `viewBox` fits. An icon that
//! fails is left out with one Console error saying why; the package
//! registers and runs without it, and a name that finds no icon draws a
//! neutral icon of Studio's (with one Console error per name).
//!
//! ```
//! use kinogaki_studio_api::icon::{self, Icon};
//! use kinogaki_studio_api::{Package, PackageId};
//! const LEAF: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 11 11" fill="#fff">
//!     <rect x="3" y="3" width="5" height="5"/></svg>"##;
//! let trees = PackageId::declared("acme.trees");
//! let package = Package::new(trees, "Trees").with_icon(Icon::new("leaf", LEAF));
//! assert!(package.icons[0].check().is_ok());
//! assert_eq!(icon::qualified(trees, "leaf"), "acme.trees/leaf");
//! assert_eq!(icon::split("acme.trees/leaf"), Some(("acme.trees", "leaf")));
//! assert_eq!(icon::split("cube"), None);
//! ```
use crate::id::PackageId;
use std::fmt;

/// The most bytes an icon's SVG document may have.
pub const MAX_BYTES: usize = 16 * 1024;

/// The most icon pixels an icon may be wide or tall (its `viewBox`): the
/// size of Studio's own icons.
pub const MAX_PIXELS: u32 = 11;

/// The longest an icon's name may be, in bytes.
pub const MAX_NAME: usize = 64;

/// One icon a package adds: its name within the package and its SVG
/// document.
#[derive(Clone, PartialEq, Eq)]
pub struct Icon {
    /// Its name within its package (`"leaf"`); others name it
    /// [`qualified`] (`"acme.trees/leaf"`).
    pub name: String,
    /// Its SVG document, as the package embeds it (`include_str!` or
    /// `include_bytes!`).
    pub svg: Vec<u8>,
}

impl Icon {
    /// Icon `name` drawn by SVG document `svg`.
    pub fn new(name: impl Into<String>, svg: impl Into<Vec<u8>>) -> Self {
        Icon {
            name: name.into(),
            svg: svg.into(),
        }
    }

    /// Its document as text, when its name is well formed and its document
    /// is UTF-8 of at most [`MAX_BYTES`] bytes. Studio then reads the
    /// document and checks its `viewBox` (see the [module](self) docs).
    pub fn check(&self) -> Result<&str, IconError> {
        if !name_well_formed(&self.name) {
            return Err(IconError::Name(self.name.clone()));
        }
        if self.svg.len() > MAX_BYTES {
            return Err(IconError::TooLarge(self.svg.len()));
        }
        std::str::from_utf8(&self.svg).map_err(|_| IconError::NotText)
    }
}

impl fmt::Debug for Icon {
    /// The icon by its name and its document's size.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Icon")
            .field("name", &self.name)
            .field("bytes", &self.svg.len())
            .finish()
    }
}

/// Why an icon was left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconError {
    /// Its name is not of the form [`name_well_formed`] describes.
    Name(String),
    /// Its document has this many bytes, more than [`MAX_BYTES`].
    TooLarge(usize),
    /// Its document is not UTF-8 text.
    NotText,
}

impl fmt::Display for IconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IconError::Name(name) => write!(
                f,
                "{name:?} is not an icon name: a lowercase letter, then lowercase letters, \
                 digits and `-`, at most {MAX_NAME} bytes"
            ),
            IconError::TooLarge(bytes) => {
                write!(f, "its SVG has {bytes} bytes, more than {MAX_BYTES}")
            }
            IconError::NotText => f.write_str("its SVG is not UTF-8 text"),
        }
    }
}

impl std::error::Error for IconError {}

/// Whether `name` is a well-formed icon name within a package: a lowercase
/// ASCII letter followed by lowercase letters, digits and `-`, at most
/// [`MAX_NAME`] bytes (`"leaf"`, `"branch-2"`), as Studio's own are.
///
/// ```
/// use kinogaki_studio_api::icon::name_well_formed;
/// assert!(name_well_formed("branch-2"));
/// assert!(!name_well_formed("Branch"));
/// assert!(!name_well_formed("acme.trees/leaf"));
/// ```
pub fn name_well_formed(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= MAX_NAME
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The name others give icon `name` of package `package`:
/// `"acme.trees/leaf"`.
pub fn qualified(package: PackageId, name: &str) -> String {
    format!("{package}/{name}")
}

/// A package icon's name as its package's id and its name within the
/// package; `None` for one of Studio's own (a name without `/`).
pub fn split(name: &str) -> Option<(&str, &str)> {
    name.split_once('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 11 11" fill="#fff"><rect x="3" y="3" width="5" height="5"/></svg>"##;

    #[test]
    fn a_well_formed_icon_checks_as_its_text() {
        assert_eq!(Icon::new("square", SQUARE).check(), Ok(SQUARE));
    }

    #[test]
    fn names_are_lowercase_words_joined_by_dashes() {
        for good in ["a", "leaf", "branch-2", "x-ray"] {
            assert!(name_well_formed(good), "{good}");
        }
        let long = "a".repeat(MAX_NAME + 1);
        for bad in [
            "",
            "Leaf",
            "2d",
            "-leaf",
            "leaf_2",
            "acme.trees/leaf",
            &long,
        ] {
            assert!(!name_well_formed(bad), "{bad}");
        }
        assert_eq!(
            Icon::new("Leaf", SQUARE).check(),
            Err(IconError::Name("Leaf".into()))
        );
    }

    #[test]
    fn documents_are_bounded_utf8_text() {
        let big = vec![b' '; MAX_BYTES + 1];
        assert_eq!(
            Icon::new("big", big).check(),
            Err(IconError::TooLarge(MAX_BYTES + 1))
        );
        assert_eq!(
            Icon::new("bytes", vec![0xff, 0xfe]).check(),
            Err(IconError::NotText)
        );
    }

    #[test]
    fn qualified_names_split_into_package_and_name() {
        let name = qualified(PackageId::declared("acme.trees"), "leaf");
        assert_eq!(split(&name), Some(("acme.trees", "leaf")));
        assert_eq!(split("cube"), None);
    }
}
