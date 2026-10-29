//! [`Kind`]: what a field holds and how it is presented, with the
//! pieces some kinds carry ([`PathChoice`], [`Chip`], [`CurveChannel`],
//! [`FileDialog`]), a material input's connection button ([`Socket`]) and
//! the slot at a field's right edge ([`Trailing`]).
use super::{Number, Text};
use kinogaki_studio_api::params::Diagnostic;

/// The type of a field and how it is presented. Each kind says which
/// [`Value`](kinogaki_studio_api::Value) variant its field holds; a kind
/// that holds none (a button) has no value.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// `Value::Number`: a length, angle, percentage, plain number, slider
    /// or stepper.
    Number(Number),
    /// `Value::Numbers` (2–4) lettered X, Y, Z, W; `lock` shows the uniform
    /// lock in that state (its click is [`Action::Lock`](super::Action::Lock)).
    Vector {
        /// How each component shows and moves.
        number: Number,
        /// The uniform lock's state, when shown.
        lock: Option<bool>,
    },
    /// `Value::Numbers` in equal fields without letters (a crop).
    Fields(Number),
    /// `Value::Numbers` (3 or 4): working-space RGB(A), a swatch and
    /// channel fields.
    Color,
    /// `Value::Numbers` (4): R, G, B, A. Linked, one slider (the value of
    /// R) sets all four; unlinked, four fields lettered R, G, B, A. The
    /// link button reports [`Action::Lock`](super::Action::Lock) with its
    /// new state; the owner keeps `linked`.
    Channels {
        /// How each channel shows and moves.
        number: Number,
        /// One value sets all four channels.
        linked: bool,
    },
    /// `Value::Index` as segments.
    Choice(Vec<Text>),
    /// `Value::Index` in a menu.
    Dropdown {
        /// The entries.
        items: Vec<Text>,
        /// Which entries are offered; empty offers all.
        available: Vec<bool>,
        /// Small text after the menu.
        info: Option<Text>,
    },
    /// `Value::Bool`.
    Toggle,
    /// `Value::Text`, committed when the edit ends.
    Text,
    /// `Value::Text`: code, many lines in the code face below its label,
    /// colored and completed by the sheet's
    /// [`CodeLanguage`](super::CodeLanguage). Edits are reported while
    /// typing, a burst of typing one gesture.
    Code {
        /// Problems to mark: those with a span are underlined.
        diagnostics: Vec<Diagnostic>,
        /// Takes the height left in the sheet instead of growing with its
        /// lines.
        fill: bool,
        /// A height in whole lines the host keeps (its editor's grip sets
        /// it, reported as [`Action::Resize`](super::Action::Resize));
        /// `None` sizes it by its text.
        lines: Option<u32>,
    },
    /// `Value::Text` that stays hidden (an access code): shown as a fixed
    /// run of bullets whatever its length, typed masked, never copied out;
    /// holding the eye button shows it.
    Secret,
    /// `Value::Text`: a file's path, typed or chosen with the folder
    /// button. The button opens the host's file dialog as `FileDialog`
    /// says; the chosen file is written into the field as an edit of its
    /// own (one undo step), stored relative to the project's folder when it
    /// lies inside it.
    Path(FileDialog),
    /// `Value::Text`: a path in the scene's tree with a picker of
    /// `choices`.
    TreePath {
        /// The entries the picker offers.
        choices: Vec<PathChoice>,
        /// The picker writes paths relative to the field's owner (its
        /// toggle reports [`Action::Relative`](super::Action::Relative)).
        relative: bool,
        /// The picker button's hover text.
        tooltip: Text,
    },
    /// `Value::Text`: an expression; with `pick`, a pick button with that
    /// hover text ([`Action::Pick`](super::Action::Pick)).
    Expression {
        /// The pick button's hover text, when there is one.
        pick: Option<Text>,
    },
    /// `Value::Text`: names, with a menu of `names` behind a button.
    Name {
        /// The names the menu offers.
        names: Vec<String>,
        /// A chosen name joins the text instead of replacing it.
        list: bool,
        /// The menu button's hover text.
        tooltip: Text,
    },
    /// `Value::Text`, read-only, right-aligned.
    Info,
    /// `Value::Text`, read-only, left-aligned.
    Value,
    /// A button ([`Action::Press`](super::Action::Press)); no value.
    Button {
        /// The button's text.
        text: Text,
        /// The name of an icon before the text.
        icon: Option<Text>,
    },
    /// Equal buttons, each enabled or not ([`Action::Index`](super::Action::Index));
    /// no value.
    Buttons(Vec<(Text, bool)>),
    /// `Value::Text` then icon chips ([`Action::Index`](super::Action::Index)).
    Chips(Vec<Chip>),
    /// A multi-input's wires in order, by the names of what they come
    /// from: rows that drag to reorder ([`Action::Reorder`](super::Action::Reorder))
    /// and remove ([`Action::Remove`](super::Action::Remove)); no value.
    Wires(Vec<Text>),
    /// `Value::Numbers`: a float or color ramp, as
    /// [`ramp::encode`](super::ramp::encode) writes it (its length fixes
    /// how many keys it holds); drawn as the ramp's curve or gradient with
    /// its keys, the selected key's fields below.
    Ramp {
        /// A color ramp rather than a float ramp.
        color: bool,
        /// Every key runs to the next this way and the keys offer no
        /// choice (a renderer's linear ramps); `None`: each key chooses.
        interpolation: Option<super::ramp::Interpolation>,
    },
    /// `Value::Numbers`: tone curves, one a channel, as
    /// [`tone_curve::encode`](super::tone_curve::encode) writes them with
    /// `capacity` keys a curve; drawn as a curve editor with a tab a
    /// channel.
    ToneCurves {
        /// The channels, in curve order: each one's tab.
        channels: Vec<CurveChannel>,
        /// The most keys a curve holds.
        capacity: usize,
    },
    /// `Value::Numbers`: a palette, r, g, b a color (display code values,
    /// 0–1, drawn as they are), at most `capacity` colors; drawn as a
    /// swatch grid. Editable: a click on a swatch edits its color, and
    /// colors can be added and removed; otherwise read-only.
    Palette {
        /// The most colors it holds.
        capacity: usize,
        /// Colors can be edited, added and removed.
        editable: bool,
    },
}

/// One channel of [`Kind::ToneCurves`]: its tab's text and its curve's
/// tint.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveChannel {
    /// The tab's text.
    pub label: Text,
    /// The curve's tint, linear RGB; `None` draws it in the text color.
    pub tint: Option<[f64; 3]>,
}

/// The file dialog of a [`Kind::Path`] field's folder button.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileDialog {
    /// Asks for a file to write (a new name may be typed) instead of one
    /// to read.
    pub save: bool,
    /// The kinds of file offered, each a name and its extensions; empty
    /// offers every file.
    pub filters: Vec<FileFilter>,
}

impl FileDialog {
    /// Asks for an existing file, of any kind until filtered.
    pub fn open() -> Self {
        FileDialog::default()
    }

    /// Asks for a file to write.
    pub fn save() -> Self {
        FileDialog {
            save: true,
            filters: Vec::new(),
        }
    }

    /// Offering files named `name` ("Images (PNG, JPEG)") with these
    /// `extensions` (without the dot).
    pub fn filter(mut self, name: impl Into<Text>, extensions: &[&'static str]) -> Self {
        self.filters.push(FileFilter {
            name: name.into(),
            extensions: extensions.iter().map(|&e| e.into()).collect(),
        });
        self
    }
}

/// One kind of file a [`FileDialog`] offers.
#[derive(Clone, Debug, PartialEq)]
pub struct FileFilter {
    /// What the dialog calls it.
    pub name: Text,
    /// Its extensions, without the dot.
    pub extensions: Vec<Text>,
}

/// The kind of value a material input carries: its connection button's
/// fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketKind {
    /// A number.
    Float,
    /// A color.
    Color,
    /// A direction or point.
    Vector,
    /// Texture coordinates.
    Uv,
    /// A shading surface.
    Surface,
}

impl SocketKind {
    /// Every kind, in the order the host lists them.
    pub const ALL: [SocketKind; 5] = [
        Self::Float,
        Self::Color,
        Self::Vector,
        Self::Uv,
        Self::Surface,
    ];
}

/// A material input's connection button before its field: unlinked, or
/// linked to a source of `kind`. Its click is
/// [`Action::Socket`](super::Action::Socket).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Socket {
    /// What the input carries (the linked source, when linked).
    pub kind: SocketKind,
    /// Whether a source is linked.
    pub linked: bool,
}

/// A tree path picker's entry: its depth in the tree, name and icon, and
/// its path written from the root and from the field's owner.
#[derive(Clone, Debug, PartialEq)]
pub struct PathChoice {
    /// Its depth in the tree (indentation).
    pub depth: usize,
    /// Its name.
    pub label: String,
    /// Its icon's name.
    pub icon: Text,
    /// Its path from the root.
    pub absolute: String,
    /// Its path from the field's owner.
    pub relative: String,
    /// Whether the field names it now.
    pub current: bool,
    /// Whether the field may name it; `false` makes it a heading for the
    /// entries below it, drawn muted.
    pub fits: bool,
    /// Why it can't be chosen now though it fits: drawn muted, with this
    /// as its hover text.
    pub unavailable: Option<Text>,
}

/// One icon chip of [`Kind::Chips`].
#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    /// The icon's name.
    pub icon: Text,
    /// Its hover text.
    pub tooltip: Text,
    /// Whether it is lit.
    pub on: bool,
}

/// The slot at a field's right edge.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Trailing {
    /// Nothing.
    #[default]
    None,
    /// An icon button ([`Action::Trailing`](super::Action::Trailing)).
    Icon {
        /// The icon's name.
        icon: Text,
        /// Its hover text.
        tooltip: Text,
        /// Whether it is lit.
        on: bool,
    },
    /// A color swatch of linear RGB.
    Swatch([f64; 3]),
}

impl Trailing {
    /// The swatch of an emission of any intensity (a light's color times
    /// its strength, HDR): its color with the brightest channel at 1.
    pub fn emission(rgb: [f64; 3]) -> Self {
        let top = rgb.iter().copied().fold(0.0, f64::max).max(1e-9);
        Trailing::Swatch(rgb.map(|v| v.max(0.0) / top))
    }
}
