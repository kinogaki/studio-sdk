//! [`Theme`]: the colors, radii and sizes the host draws with, as plain
//! data the host fills in, so a plugin's own drawing matches the host's.
use crate::color::Color;
use crate::geom::Vec2;

/// The host's style tokens. [`Ui::theme`](crate::Ui::theme) hands out
/// the host's; [`Theme::default`] holds Kinogaki's reference values (at
/// the default element size), which tests and fakes draw with.
///
/// Colors go from the darkest grey to the lightest, then the accents.
/// Use the token that names the role (`surface` for a card, `muted` for
/// a hint), not one that happens to look right, so a plugin follows the
/// host when its theme changes.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Panel and canvas background, title bars.
    pub backdrop: Color,
    /// Pane header strips, tab strips.
    pub strip: Color,
    /// Text fields and resting buttons.
    pub field: Color,
    /// Floating windows and shelves.
    pub raised: Color,
    /// Popups, cards, tiles and the inspector.
    pub surface: Color,
    /// Hairlines and hovered fills.
    pub border: Color,
    /// Pressed buttons.
    pub pressed: Color,
    /// Headings, hints and unavailable items.
    pub muted: Color,
    /// Text and icons.
    pub text: Color,
    /// Ink of disabled values (and their fields' outlines).
    pub disabled_ink: Color,
    /// The selection, the active tool, tab or row, value pills.
    pub accent: Color,
    /// Errors and warnings.
    pub error: Color,
    /// A frozen node's flag.
    pub frozen: Color,
    /// The one of several alternatives in effect (not the selection).
    pub active: Color,
    /// Template objects: their tint and flag.
    pub template: Color,
    /// The thin outline under text drawn straight onto the viewport, which
    /// keeps it readable on light and dark surfaces.
    pub label_halo: Color,
    /// The X, Y and Z axes.
    pub axes: [Color; 3],
    /// Edge marks in the viewport.
    pub marks: Marks,
    /// Corner radii.
    pub radius: Radii,
    /// Spacing, rows and control sizes.
    pub metrics: Metrics,
    /// The interface text size, in points: [`Font::ui`](crate::Font::ui)'s.
    pub text_size: f32,
}

/// The colors of edge marks in the viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marks {
    /// Crease weight.
    pub crease: Color,
    /// UV seams.
    pub seam: Color,
    /// Hard (sharp) edges.
    pub hard: Color,
    /// Bevel weight.
    pub bevel: Color,
}

/// Corner radii, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii {
    /// List and browser rows.
    pub row: f32,
    /// Small floating chips and hints.
    pub chip: f32,
    /// Network node bodies.
    pub node: f32,
    /// Icon tiles, tabs, marking-menu rows, value pills.
    pub tile: f32,
    /// Buttons, fields and popup windows.
    pub control: f32,
    /// Docked panes and floating cards.
    pub pane: f32,
}

/// Spacing, rows and control sizes, in points, at the user's element size
/// (the host scales them; a plugin reads them rather than hard-coding).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// The space between controls.
    pub gap: f32,
    /// Padding inside text buttons.
    pub button_padding: Vec2,
    /// The smallest interactive control (fields, text buttons).
    pub interact_size: Vec2,
    /// Square icon buttons.
    pub icon_button: f32,
    /// An icon glyph.
    pub icon_size: f32,
    /// Slider-like value fields' width.
    pub slider_width: f32,
    /// A property sheet's label column.
    pub property_label: f32,
    /// A property row.
    pub property_row: f32,
    /// A property section band.
    pub section_row: f32,
    /// A property subsection band.
    pub subsection_row: f32,
    /// A floating card's rows.
    pub card_row: f32,
    /// Marking-menu rows.
    pub menu_row: f32,
    /// Browser rows.
    pub list_row: f32,
    /// Tree rows (an outliner).
    pub tree_row: f32,
    /// Tab strips.
    pub tab_height: f32,
}

impl Default for Theme {
    /// Kinogaki's reference theme at the default element size.
    fn default() -> Self {
        let interact = Vec2::new(24.0, 22.0);
        Self {
            backdrop: Color::gray(26),
            strip: Color::gray(38),
            field: Color::gray(41),
            raised: Color::gray(46),
            surface: Color::gray(50),
            border: Color::gray(58),
            pressed: Color::gray(72),
            muted: Color::gray(140),
            text: Color::gray(217),
            disabled_ink: Color::gray(110),
            accent: Color::rgb(231, 160, 63),
            error: Color::rgb(222, 72, 62),
            frozen: Color::rgb(150, 48, 44),
            active: Color::rgb(104, 196, 84),
            template: Color::rgb(168, 118, 240),
            label_halo: Color::premultiplied(0, 0, 0, 200),
            axes: [
                Color::rgb(201, 99, 83),
                Color::rgb(137, 176, 105),
                Color::rgb(101, 151, 194),
            ],
            marks: Marks {
                crease: Color::rgb(214, 72, 168),
                seam: Color::rgb(226, 64, 40),
                hard: Color::rgb(64, 200, 220),
                bevel: Color::rgb(70, 140, 240),
            },
            radius: Radii {
                row: 2.0,
                chip: 3.0,
                node: 3.0,
                tile: 4.0,
                control: 5.0,
                pane: 6.0,
            },
            metrics: Metrics {
                gap: 2.0,
                button_padding: Vec2::new(7.0, 3.0),
                interact_size: interact,
                icon_button: 28.0,
                icon_size: 22.0,
                slider_width: 120.0,
                property_label: 104.0,
                property_row: 24.0,
                section_row: 24.0,
                subsection_row: 20.0,
                card_row: 26.0,
                menu_row: 26.0,
                list_row: 28.0,
                tree_row: 24.0,
                tab_height: 24.0,
            },
            text_size: 15.0,
        }
    }
}

impl Theme {
    /// The ink for text and icons drawn on `fill`: black on a light fill
    /// (the accent, [`Theme::active`], any color black reads better on),
    /// [`Theme::text`] on a dark one. Ask here whenever you ink a fill, so
    /// text and icons never disagree with the host's.
    ///
    /// The choice is the ink with the higher WCAG contrast against the
    /// fill; a translucent fill is judged as laid over black.
    pub fn ink_on(&self, fill: Color) -> Color {
        if self.dark_ink_on(fill) {
            Color::BLACK
        } else {
            self.text
        }
    }

    /// The ink for disabled or secondary text and icons on `fill`: on a
    /// light fill, the fill darkened most of the way to black; on a dark
    /// one, [`Theme::disabled_ink`].
    pub fn dim_ink_on(&self, fill: Color) -> Color {
        if self.dark_ink_on(fill) {
            fill.blend(Color::black_alpha(150)).to_opaque()
        } else {
            self.disabled_ink
        }
    }

    /// Whether black contrasts with `fill` at least as well as the text
    /// color does.
    fn dark_ink_on(&self, fill: Color) -> bool {
        let (fill, text) = (fill.luminance() + 0.05, self.text.luminance() + 0.05);
        fill / 0.05 >= text / fill
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_fills_take_dark_ink_and_dark_fills_light() {
        let theme = Theme::default();
        for fill in [theme.accent, theme.active, Color::rgb(0xD9, 0x51, 0x9F)] {
            assert_eq!(theme.ink_on(fill), Color::BLACK, "{fill:?}");
            let dim = theme.dim_ink_on(fill);
            assert!(
                dim.luminance() < fill.luminance() / 2.0,
                "{fill:?}: {dim:?}"
            );
            assert!(
                dim.luminance() > Color::BLACK.luminance(),
                "dimmed, not black"
            );
        }
        for fill in [
            theme.backdrop,
            theme.strip,
            theme.field,
            theme.raised,
            theme.surface,
            theme.border,
            theme.pressed,
            theme.frozen,
            Color::TRANSPARENT,
            // A translucent accent tint over the panels.
            theme.accent.linear_multiply(0.25),
        ] {
            assert_eq!(theme.ink_on(fill), theme.text, "{fill:?}");
            assert_eq!(theme.dim_ink_on(fill), theme.disabled_ink, "{fill:?}");
        }
    }

    #[test]
    fn the_ink_follows_the_theme_text() {
        let theme = Theme {
            text: Color::gray(255),
            ..Theme::default()
        };
        assert_eq!(theme.ink_on(theme.backdrop), Color::gray(255));
    }
}
