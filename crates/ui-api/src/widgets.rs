//! What a plugin asks of the host's widgets: [`ButtonSpec`],
//! [`TextEditSpec`], [`MenuItem`] and [`ChoiceLook`]. Plain data, borrowed
//! for the one call.
use crate::color::Color;
use crate::geom::Vec2;
use crate::id::Id;

/// A button: text, an icon or both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonSpec<'a> {
    /// Its text; empty for an icon button.
    pub text: &'a str,
    /// The name of an icon before the text, from the host's icon set.
    pub icon: Option<&'a str>,
    /// Drawn lit (the active tool, a toggled option).
    pub selected: bool,
    /// Takes clicks; a disabled button draws greyed.
    pub enabled: bool,
    /// Hover text.
    pub tooltip: Option<&'a str>,
    /// Its smallest size; zero leaves it to the host.
    pub min_size: Vec2,
    /// Its fill instead of the host's; the ink follows
    /// [`Theme::ink_on`](crate::Theme::ink_on).
    pub fill: Option<Color>,
    /// Drawn without a frame until hovered.
    pub frameless: bool,
}

impl<'a> ButtonSpec<'a> {
    /// A text button.
    pub fn new(text: &'a str) -> Self {
        ButtonSpec {
            text,
            icon: None,
            selected: false,
            enabled: true,
            tooltip: None,
            min_size: Vec2::ZERO,
            fill: None,
            frameless: false,
        }
    }

    /// An icon button: `name` from the host's icon set, with `tooltip`.
    pub fn icon(name: &'a str, tooltip: &'a str) -> Self {
        ButtonSpec {
            icon: Some(name),
            tooltip: Some(tooltip),
            ..ButtonSpec::new("")
        }
    }

    /// With the icon `name` before its text.
    pub fn with_icon(mut self, name: &'a str) -> Self {
        self.icon = Some(name);
        self
    }

    /// Lit, or not.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Enabled, or not.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// With hover text.
    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    /// At least `size`.
    pub fn min_size(mut self, size: Vec2) -> Self {
        self.min_size = size;
        self
    }

    /// Filled with `fill`.
    pub fn fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Without a frame until hovered.
    pub fn frameless(mut self) -> Self {
        self.frameless = true;
        self
    }
}

/// A text field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextEditSpec<'a> {
    /// Its id, to keep its focus and cursor when the layout around it
    /// changes, or to [request the focus](crate::Ui::request_focus); `None`
    /// lets the host place it.
    pub id: Option<Id>,
    /// Grey text shown while it is empty.
    pub hint: &'a str,
    /// Its width; `None` takes the available width.
    pub width: Option<f32>,
    /// Several lines (Enter starts a new line).
    pub multiline: bool,
    /// In the code face.
    pub code: bool,
    /// Takes input; a disabled field shows its text greyed.
    pub enabled: bool,
}

impl<'a> TextEditSpec<'a> {
    /// A one-line, enabled field across the available width.
    pub fn new() -> Self {
        TextEditSpec {
            id: None,
            hint: "",
            width: None,
            multiline: false,
            code: false,
            enabled: true,
        }
    }

    /// With id `id`.
    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    /// With `hint` shown while empty.
    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    /// `width` points wide.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Several lines.
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    /// In the code face.
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }

    /// Enabled, or not.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Default for TextEditSpec<'_> {
    /// [`TextEditSpec::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// An entry of a menu. Clicking one closes the menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuItem<'a> {
    /// Its text.
    pub text: &'a str,
    /// The shortcut shown at its right (`"⌘D"`).
    pub shortcut: Option<&'a str>,
    /// A check mark's state, for an entry that toggles.
    pub checked: Option<bool>,
    /// Takes clicks.
    pub enabled: bool,
    /// The name of an icon before the text.
    pub icon: Option<&'a str>,
}

impl<'a> MenuItem<'a> {
    /// An enabled entry.
    pub fn new(text: &'a str) -> Self {
        MenuItem {
            text,
            shortcut: None,
            checked: None,
            enabled: true,
            icon: None,
        }
    }

    /// Showing `shortcut`.
    pub fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// With a check mark in state `checked`.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Enabled, or not.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// With the icon `name`.
    pub fn icon(mut self, name: &'a str) -> Self {
        self.icon = Some(name);
        self
    }
}

/// How a [choice](crate::Ui::choice) between a few entries shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChoiceLook {
    /// Side-by-side segments, one lit.
    #[default]
    Segments,
    /// A dropdown menu.
    Dropdown,
}
