//! [`Kind`]: what a field holds and how it is presented, with the
//! pieces some kinds carry ([`PathChoice`], [`Chip`]) and the slot at a
//! field's right edge ([`Trailing`]).
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
    /// `Value::Text` with a folder button ([`Action::Browse`](super::Action::Browse)).
    Path,
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
    /// `Value::Numbers`: an encoded ramp (the key count, then position,
    /// value and interpolation code for each key) between `min` and `max`.
    Ramp {
        /// The lowest value of the ramp's range.
        min: f64,
        /// The highest value of the ramp's range.
        max: f64,
        /// The ramp sampled evenly from 0 to 1, for the plot; empty lets the
        /// host sample the value.
        curve: Vec<f64>,
        /// The interpolation codes' names.
        interpolations: Vec<Text>,
    },
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
