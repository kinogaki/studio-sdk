//! A context's display options ([`DisplayOption`]): what its views may
//! show besides the value itself (labels, guides, a coloring), declared as
//! data in its [`ViewportSpec`](super::ViewportSpec). The host keeps each
//! option's value, draws its row in each view's Display popover after
//! core's own rows, and answers a view call's
//! [`ViewCx::displayed`](crate::host::ViewCx::displayed); the context draws
//! what the option shows (its [`Display`](super::Display): `edited` or `overlay`).
//!
//! A value is the host's stored state of the option's scope: a
//! [`PerView`](DisplayScope::PerView) option is each view's own, saved with
//! the project's view; a [`Context`](DisplayScope::Context) one is the
//! user's, the same in every view and project. Neither is undone, and
//! nothing cooks when one changes.
//!
//! ```
//! use studio_ui_api::package::{DisplayKind, DisplayOption, DisplayScope, ViewportSpec};
//!
//! const LEAVES: DisplayOption = DisplayOption {
//!     id: "acme.trees.leaves",
//!     label: "Leaves",
//!     scope: DisplayScope::PerView,
//!     kind: DisplayKind::Toggle { on: true },
//! };
//! let spec = ViewportSpec { display: vec![LEAVES], ..ViewportSpec::scene() };
//! assert_eq!(spec.option(LEAVES.id).map(DisplayOption::default_value), Some(1));
//! ```

/// One display option of a context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayOption {
    /// Its id: unique among installed packages' options, its package's
    /// name first ("acme.trees.leaves"). Contexts declaring the same id
    /// share its value.
    pub id: &'static str,
    /// Its row's label ("Leaves").
    pub label: &'static str,
    /// Whose value it is.
    pub scope: DisplayScope,
    /// What it holds, and its value where the user never set it.
    pub kind: DisplayKind,
}

/// Whose value a display option is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DisplayScope {
    /// Each view's own (saved with the project's view).
    PerView,
    /// The context's, the same in every view (saved with the user's
    /// settings).
    Context,
}

/// What a display option holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayKind {
    /// On or off (its value 1 or 0), starting `on`.
    Toggle {
        /// Its default.
        on: bool,
    },
    /// One of `options` (its value the chosen one's index), starting at
    /// `chosen`.
    Choice {
        /// The choices' labels.
        options: &'static [&'static str],
        /// Its default.
        chosen: usize,
    },
}

impl DisplayOption {
    /// Its value where the user never set it: a toggle's 1 or 0, a
    /// choice's index.
    pub fn default_value(&self) -> usize {
        match self.kind {
            DisplayKind::Toggle { on } => usize::from(on),
            DisplayKind::Choice { chosen, .. } => chosen,
        }
    }

    /// Whether `value` is one it can hold.
    pub fn holds(&self, value: usize) -> bool {
        match self.kind {
            DisplayKind::Toggle { .. } => value <= 1,
            DisplayKind::Choice { options, .. } => value < options.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_holds_its_options_and_a_toggle_two_values() {
        let coloring = DisplayOption {
            id: "acme.trees.coloring",
            label: "Color by",
            scope: DisplayScope::Context,
            kind: DisplayKind::Choice {
                options: &["Age", "Height"],
                chosen: 1,
            },
        };
        assert_eq!(coloring.default_value(), 1);
        assert!(coloring.holds(1) && !coloring.holds(2));
        let toggle = DisplayOption {
            kind: DisplayKind::Toggle { on: false },
            ..coloring
        };
        assert_eq!(toggle.default_value(), 0);
        assert!(toggle.holds(1) && !toggle.holds(2));
    }
}
