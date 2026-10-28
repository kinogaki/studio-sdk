//! Settings: the fields every render reads ([`RenderSettings`]) and the
//! values an engine describes itself ([`SettingDescriptor`]s, kept by
//! name). The same descriptors describe the parameters of the prim types an
//! engine declares ([`crate::PrimType`]): a light's size, a fog's density.
//! A descriptor says what a value holds, how a UI edits it, and when it is
//! shown or applies, so a host draws any engine's rows without knowing it.

use std::sync::Arc;

use crate::id::{PrimId, TextureId};
use crate::vocabulary::Choice;

/// A setting's value.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingValue {
    /// On or off.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A number.
    Float(f64),
    /// One of a descriptor's choices, by name.
    Choice(Arc<str>),
    /// A color, linear ACEScg.
    Color([f64; 3]),
    /// A color ramp's stops, by ascending position.
    ColorRamp(Arc<[ColorStop]>),
    /// Two numbers (an offset across a plane).
    Vector2([f64; 2]),
    /// An image the host decoded and sent as a Texture prim (none: no
    /// image). Hosts set it; UIs do not edit it.
    Texture(Option<TextureId>),
}

impl SettingValue {
    /// A toggle's state.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            SettingValue::Bool(on) => Some(*on),
            _ => None,
        }
    }

    /// A whole number.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            SettingValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// A number.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            SettingValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    /// A choice's name.
    pub fn as_choice(&self) -> Option<&str> {
        match self {
            SettingValue::Choice(name) => Some(name),
            _ => None,
        }
    }

    /// A color.
    pub fn as_color(&self) -> Option<[f64; 3]> {
        match self {
            SettingValue::Color(c) => Some(*c),
            _ => None,
        }
    }

    /// Two numbers.
    pub fn as_vector2(&self) -> Option<[f64; 2]> {
        match self {
            SettingValue::Vector2(v) => Some(*v),
            _ => None,
        }
    }

    /// A texture.
    pub fn as_texture(&self) -> Option<Option<TextureId>> {
        match self {
            SettingValue::Texture(t) => Some(*t),
            _ => None,
        }
    }
}

/// The most stops a color ramp holds.
pub const MAX_RAMP_STOPS: usize = 16;

/// A color ramp's stop: a color (linear ACEScg) at a position in 0..=1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorStop {
    /// Where it sits, 0 to 1.
    pub position: f64,
    /// Its color.
    pub color: [f64; 3],
}

impl ColorStop {
    /// The stop `color` at `position`.
    pub const fn new(position: f64, color: [f64; 3]) -> ColorStop {
        ColorStop { position, color }
    }
}

/// Ramp `stops` (ascending, at least one) at `t`: linear between the stops
/// around it, the end stops' colors beyond them.
pub fn ramp_color(stops: &[ColorStop], t: f64) -> [f64; 3] {
    let Some(first) = stops.first() else {
        return [0.0; 3];
    };
    if t.is_nan() || t <= first.position {
        return first.color;
    }
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t <= b.position {
            let span = b.position - a.position;
            let u = if span > 0.0 {
                (t - a.position) / span
            } else {
                1.0
            };
            return [0, 1, 2].map(|i| a.color[i] + (b.color[i] - a.color[i]) * u);
        }
    }
    stops[stops.len() - 1].color
}

/// Whether `stops` make a ramp: 1 to [`MAX_RAMP_STOPS`] stops, positions
/// ascending in 0..=1, colors finite and not negative.
fn is_ramp(stops: &[ColorStop]) -> bool {
    (1..=MAX_RAMP_STOPS).contains(&stops.len())
        && stops.windows(2).all(|p| p[0].position <= p[1].position)
        && stops
            .iter()
            .all(|s| (0.0..=1.0).contains(&s.position) && is_color(&s.color))
}

fn is_color(c: &[f64; 3]) -> bool {
    c.iter().all(|v| v.is_finite() && *v >= 0.0)
}

/// Engine setting values, by descriptor name.
pub type SettingValues = Vec<(Arc<str>, SettingValue)>;

/// Value `name` in `values`.
pub fn value<'a>(values: &'a SettingValues, name: &str) -> Option<&'a SettingValue> {
    values.iter().find(|(n, _)| &**n == name).map(|(_, v)| v)
}

/// Set value `name` in `values`.
pub fn set_value(values: &mut SettingValues, name: &str, new: SettingValue) {
    match values.iter_mut().find(|(n, _)| &**n == name) {
        Some((_, v)) => *v = new,
        None => values.push((Arc::from(name), new)),
    }
}

/// What a render reads: the fields every engine needs, and the engine's
/// own values by name.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSettings {
    /// Final-frame width and height, in pixels.
    pub resolution: [u32; 2],
    /// The Camera prim final frames render through (none: the view's camera).
    pub camera: Option<PrimId>,
    /// Millimetres in one scene unit (1000 for metres): what real sizes in
    /// materials (an anodized part's grain) are measured against.
    pub mm_per_unit: f32,
    /// The world's up direction (unit): what "up" means to sky light and
    /// height fog.
    pub up: [f32; 3],
    /// The engine's settings, by its descriptors' names; one missing takes
    /// its descriptor's default.
    pub values: SettingValues,
}

impl Default for RenderSettings {
    fn default() -> RenderSettings {
        RenderSettings {
            resolution: [1920, 1080],
            camera: None,
            mm_per_unit: 1000.0,
            up: [0.0, 1.0, 0.0],
            values: Vec::new(),
        }
    }
}

impl RenderSettings {
    /// Value `name`, as set.
    pub fn value(&self, name: &str) -> Option<&SettingValue> {
        value(&self.values, name)
    }

    /// Set value `name`.
    pub fn set(&mut self, name: &str, value: SettingValue) {
        set_value(&mut self.values, name, value);
    }
}

/// The type, default and range of a setting.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingKind {
    /// On or off.
    Bool {
        /// Its default.
        default: bool,
    },
    /// A whole number in `min..=max`.
    Int {
        /// Its default.
        default: i64,
        /// The smallest.
        min: i64,
        /// The largest.
        max: i64,
    },
    /// A number in `min..=max`: a slider over `soft`, or a field dragged
    /// `step` a pixel.
    Float {
        /// Its default.
        default: f64,
        /// The smallest.
        min: f64,
        /// The largest.
        max: f64,
        /// The range a slider (or an angle's drag) covers.
        soft: (f64, f64),
        /// Decimals shown.
        decimals: u8,
        /// What it measures.
        unit: Unit,
        /// A slider rather than a dragged field.
        slider: bool,
        /// How much a dragged field changes a pixel.
        step: f64,
    },
    /// Two numbers, each in `min..=max`, dragged `step` a pixel.
    Vector2 {
        /// Its default.
        default: [f64; 2],
        /// The smallest.
        min: f64,
        /// The largest.
        max: f64,
        /// What they measure.
        unit: Unit,
        /// How much a drag changes them a pixel.
        step: f64,
    },
    /// An image the host decodes and sends (none by default); never shown.
    Texture,
    /// One of named choices; `menu` shows them as a dropdown (else as
    /// segments).
    Choice {
        /// The choices.
        choices: Vec<Choice>,
        /// The default's name.
        default: Arc<str>,
        /// A dropdown rather than segments.
        menu: bool,
    },
    /// A color (linear ACEScg, channels not negative).
    Color {
        /// Its default.
        default: [f64; 3],
    },
    /// A color ramp: stops from 0 to 1 ([`ramp_color`] reads it).
    ColorRamp {
        /// Its default stops.
        default: Arc<[ColorStop]>,
    },
}

/// What a number measures, for how a UI shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Unit {
    /// A plain number.
    #[default]
    None,
    /// A length in scene units: hosts show it in their length unit, with
    /// that unit's decimals.
    Length,
    /// An angle in degrees.
    Angle,
    /// A number shown with this text after it (" K", " EV").
    Suffix(Arc<str>),
}

/// A setting holding a value: what [`Visibility::When`] and a group's
/// [`SettingGroup::open_with`] test.
#[derive(Clone, Debug, PartialEq)]
pub struct Condition {
    /// The setting tested.
    pub setting: Arc<str>,
    /// The value it holds.
    pub value: SettingValue,
}

impl Condition {
    /// Setting `setting` holding `value`.
    pub fn new(setting: &str, value: SettingValue) -> Condition {
        Condition {
            setting: Arc::from(setting),
            value,
        }
    }

    /// Whether `values` meet it (a setting missing from them does not).
    pub fn met(&self, values: &SettingValues) -> bool {
        value(values, &self.setting) == Some(&self.value)
    }
}

/// When a UI shows a setting's row.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Visibility {
    /// Always.
    #[default]
    Always,
    /// While a setting holds a value (a rectangle's width).
    When(Condition),
    /// Never: the host sets it (a dome's image).
    Never,
}

/// What a UI shows after a setting's row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trailing {
    /// The color the standard emission settings beside it make
    /// ([`crate::standard::emission_params`]), brightest channel at 1: the
    /// host works it out (an illuminant's or blackbody's color needs its
    /// spectrum).
    EmissionSwatch,
}

/// When a setting applies: while setting `setting` holds `value` (else a UI
/// shows it disabled, saying `reason`).
#[derive(Clone, Debug, PartialEq)]
pub struct Requirement {
    /// The setting it depends on.
    pub setting: Arc<str>,
    /// The value it needs.
    pub value: SettingValue,
    /// Why it does not apply otherwise.
    pub reason: Arc<str>,
}

/// A group of settings: a section of the inspector.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingGroup {
    /// Its name, as descriptors name it.
    pub name: Arc<str>,
    /// Its title.
    pub label: Arc<str>,
    /// Whether it starts open.
    pub open: bool,
    /// A toggle that opens it when on, whatever `open` says (a gobo's
    /// section).
    pub open_with: Option<Arc<str>>,
}

impl SettingGroup {
    /// Group `name` titled `label`, open when `open`.
    pub fn new(name: &str, label: &str, open: bool) -> SettingGroup {
        SettingGroup {
            name: Arc::from(name),
            label: Arc::from(label),
            open,
            open_with: None,
        }
    }

    /// Whether it starts open, its settings holding `values`.
    pub fn is_open(&self, values: &SettingValues) -> bool {
        self.open
            || self
                .open_with
                .as_ref()
                .is_some_and(|t| value(values, t) == Some(&SettingValue::Bool(true)))
    }
}

/// A setting an engine publishes, so a UI can show it without knowing it.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingDescriptor {
    /// The key in [`RenderSettings::values`] and in files.
    pub name: Arc<str>,
    /// What a UI labels it.
    pub label: Arc<str>,
    /// Its type, default and range.
    pub kind: SettingKind,
    /// The group it sits in (a [`SettingGroup::name`]).
    pub group: Arc<str>,
    /// What it means, where its label alone does not say.
    pub tooltip: Option<Arc<str>>,
    /// When it applies (none: always).
    pub requires: Option<Requirement>,
    /// When its row is shown.
    pub visible: Visibility,
    /// What its row shows after it.
    pub trailing: Option<Trailing>,
}

impl SettingDescriptor {
    /// Setting `name`, labelled `label`, of `kind`, in group `group`:
    /// always shown and applying, without a tooltip.
    pub fn new(name: &str, label: &str, group: &str, kind: SettingKind) -> SettingDescriptor {
        SettingDescriptor {
            name: Arc::from(name),
            label: Arc::from(label),
            kind,
            group: Arc::from(group),
            tooltip: None,
            requires: None,
            visible: Visibility::Always,
            trailing: None,
        }
    }

    /// It, with `tooltip`.
    pub fn tooltip(mut self, tooltip: &str) -> SettingDescriptor {
        self.tooltip = Some(Arc::from(tooltip));
        self
    }

    /// It, applying while `setting` holds `value` (else disabled, saying
    /// `reason`).
    pub fn requires(
        mut self,
        setting: &str,
        value: SettingValue,
        reason: &str,
    ) -> SettingDescriptor {
        self.requires = Some(Requirement {
            setting: Arc::from(setting),
            value,
            reason: Arc::from(reason),
        });
        self
    }

    /// It, shown while `setting` holds `value`.
    pub fn shown_when(mut self, setting: &str, value: SettingValue) -> SettingDescriptor {
        self.visible = Visibility::When(Condition::new(setting, value));
        self
    }

    /// It, never shown.
    pub fn hidden(mut self) -> SettingDescriptor {
        self.visible = Visibility::Never;
        self
    }

    /// It, with `trailing` after its row.
    pub fn trailing(mut self, trailing: Trailing) -> SettingDescriptor {
        self.trailing = Some(trailing);
        self
    }

    /// Whether its row shows while the settings hold `values`.
    pub fn is_shown(&self, values: &SettingValues) -> bool {
        match &self.visible {
            Visibility::Always => true,
            Visibility::When(c) => c.met(values),
            Visibility::Never => false,
        }
    }

    /// Whether it applies while the settings hold `values`.
    pub fn applies(&self, values: &SettingValues) -> bool {
        self.requires
            .as_ref()
            .is_none_or(|r| value(values, &r.setting) == Some(&r.value))
    }

    /// Its default value.
    pub fn default_value(&self) -> SettingValue {
        match &self.kind {
            SettingKind::Bool { default } => SettingValue::Bool(*default),
            SettingKind::Int { default, .. } => SettingValue::Int(*default),
            SettingKind::Float { default, .. } => SettingValue::Float(*default),
            SettingKind::Choice { default, .. } => SettingValue::Choice(default.clone()),
            SettingKind::Color { default } => SettingValue::Color(*default),
            SettingKind::ColorRamp { default } => SettingValue::ColorRamp(default.clone()),
            SettingKind::Vector2 { default, .. } => SettingValue::Vector2(*default),
            SettingKind::Texture => SettingValue::Texture(None),
        }
    }

    /// Whether `value` is of its type and in its range.
    pub fn accepts(&self, value: &SettingValue) -> bool {
        match (&self.kind, value) {
            (SettingKind::Bool { .. }, SettingValue::Bool(_)) => true,
            (SettingKind::Int { min, max, .. }, SettingValue::Int(v)) => (min..=max).contains(&v),
            (SettingKind::Float { min, max, .. }, SettingValue::Float(v)) => {
                v.is_finite() && (min..=max).contains(&v)
            }
            (SettingKind::Choice { choices, .. }, SettingValue::Choice(name)) => {
                choices.iter().any(|c| c.name == *name)
            }
            (SettingKind::Color { .. }, SettingValue::Color(c)) => is_color(c),
            (SettingKind::ColorRamp { .. }, SettingValue::ColorRamp(stops)) => is_ramp(stops),
            (SettingKind::Vector2 { min, max, .. }, SettingValue::Vector2(v)) => {
                v.iter().all(|c| c.is_finite() && (min..=max).contains(&c))
            }
            (SettingKind::Texture, SettingValue::Texture(_)) => true,
            _ => false,
        }
    }
}

/// Why setting values do not fit their descriptors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingError {
    /// No descriptor has this name.
    Unknown(Arc<str>),
    /// The value is of another type or out of range.
    Invalid(Arc<str>),
}

impl std::fmt::Display for SettingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingError::Unknown(n) => write!(f, "unknown setting {n:?}"),
            SettingError::Invalid(n) => write!(f, "setting {n:?} is out of range"),
        }
    }
}

/// Every descriptor's default, in their order.
pub fn defaults(descriptors: &[SettingDescriptor]) -> SettingValues {
    descriptors
        .iter()
        .map(|d| (d.name.clone(), d.default_value()))
        .collect()
}

/// `values` made to fit `descriptors`: each descriptor's value from
/// `values` when it fits, else its default, in the descriptors' order.
/// Values no descriptor names are dropped.
pub fn fit(descriptors: &[SettingDescriptor], values: &SettingValues) -> SettingValues {
    descriptors
        .iter()
        .map(|d| {
            let v = value(values, &d.name)
                .filter(|v| d.accepts(v))
                .cloned()
                .unwrap_or_else(|| d.default_value());
            (d.name.clone(), v)
        })
        .collect()
}

/// Check `values` against `descriptors`: each names one and fits it.
pub fn validate(
    descriptors: &[SettingDescriptor],
    values: &SettingValues,
) -> Result<(), SettingError> {
    for (name, value) in values {
        let d = descriptors
            .iter()
            .find(|d| d.name == *name)
            .ok_or_else(|| SettingError::Unknown(name.clone()))?;
        if !d.accepts(value) {
            return Err(SettingError::Invalid(name.clone()));
        }
    }
    Ok(())
}
