//! Fonts and icons by role and name: the host owns the faces and the
//! icon set; a plugin says what it means.
use crate::color::Color;

/// Which face text draws in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontRole {
    /// The interface face.
    #[default]
    Ui,
    /// The code face: every glyph the same width, so columns line up.
    Code,
}

/// A face and a size in points. The host may snap the size to the sizes
/// its face is drawn at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Font {
    /// The face.
    pub role: FontRole,
    /// The size, in points.
    pub size: f32,
}

impl Font {
    /// The interface face at `size` points (the theme's
    /// [`text_size`](crate::Theme::text_size) is the usual one).
    pub const fn ui(size: f32) -> Self {
        Font {
            role: FontRole::Ui,
            size,
        }
    }

    /// The code face at `size` points.
    pub const fn code(size: f32) -> Self {
        Font {
            role: FontRole::Code,
            size,
        }
    }
}

/// How a [`Ui::label`](crate::Ui::label) draws its text. The default is
/// the interface font in the theme's text color, on one line.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextStyle {
    /// The font; `None` is the interface font at the theme's size.
    pub font: Option<Font>,
    /// The color; `None` is the theme's text color.
    pub color: Option<Color>,
    /// Wrap at the available width instead of running on.
    pub wrap: bool,
}

impl TextStyle {
    /// In `font`.
    pub fn font(mut self, font: Font) -> Self {
        self.font = Some(font);
        self
    }

    /// In `color`.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Wrapped at the available width.
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }
}
