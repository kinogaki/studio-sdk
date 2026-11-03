//! Settings: the fields every render reads ([`RenderSettings`]) and the
//! values an engine describes itself ([`SettingDescriptor`]s, kept by
//! name). The same descriptors describe the parameters of the prim types an
//! engine declares ([`crate::PrimType`]): a light's size, a fog's density.
//! A descriptor says what a value holds ([`ValueKind`], the SDK's one
//! value model), and when it is shown or applies, so a host draws any
//! engine's rows without knowing it.
//!
//! Values are authored [`Value`]s by name ([`Parameters`]). An image value
//! of a declared prim or a material node ([`ValueKind::File`]) is authored
//! as its file's path; an engine receives the Texture prim the host decoded
//! it to, in the prim's or node's [`Textures`].

use kinogaki_geometry::Units;
use kinogaki_value::{Condition, Parameters, Value, ValueKind};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::id::{PrimId, TextureId};

/// The Texture prims the host decoded image values to, by the value's
/// name: what an engine reads for each [`ValueKind::File`] value (one
/// missing: no image, or one not read yet).
pub type Textures = BTreeMap<String, TextureId>;

/// What a render reads: the fields every engine needs, and the engine's
/// own values by name.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSettings {
    /// Final-frame width and height, in pixels.
    pub resolution: [u32; 2],
    /// The Camera prim final frames render through (none: the view's camera).
    pub camera: Option<PrimId>,
    /// The scene's units: what real sizes (a lens's millimetres, an
    /// anodized part's grain) are measured against, and the axis that is
    /// up to sky light and height fog.
    pub units: Units,
    /// The engine's settings, by its descriptors' names; one missing takes
    /// its descriptor's default.
    pub values: Parameters,
}

impl Default for RenderSettings {
    fn default() -> RenderSettings {
        RenderSettings {
            resolution: [1920, 1080],
            camera: None,
            units: Units::METERS,
            values: Parameters::new(),
        }
    }
}

/// What a UI shows after a setting's row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trailing {
    /// The color the standard emission settings beside it make
    /// (`crate::standard::emission_params`), brightest channel at 1: the
    /// host works it out (an illuminant's or blackbody's color needs its
    /// spectrum).
    EmissionSwatch,
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
    /// Open whatever `open` says while this holds (a gobo's section while
    /// its toggle is on).
    pub(crate) open_when: Option<Condition>,
}

impl SettingGroup {
    /// Group `name` titled `label`, open when `open`.
    pub fn new(name: &str, label: &str, open: bool) -> SettingGroup {
        SettingGroup {
            name: Arc::from(name),
            label: Arc::from(label),
            open,
            open_when: None,
        }
    }

    /// Whether it starts open, its settings holding `values`.
    pub fn is_open(&self, values: &Parameters) -> bool {
        self.open || self.open_when.as_ref().is_some_and(|c| c.met(values))
    }
}

/// A setting an engine publishes, so a UI can show it without knowing it.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingDescriptor {
    /// The key in [`RenderSettings::values`] and in files.
    pub name: Arc<str>,
    /// What a UI labels it.
    pub label: Arc<str>,
    /// What it holds.
    pub kind: ValueKind,
    /// Its default, a value its kind takes.
    pub default: Value,
    /// The group it sits in (a [`SettingGroup::name`]).
    pub group: Arc<str>,
    /// What it means, where its label alone does not say.
    pub tooltip: Option<Arc<str>>,
    /// When it applies, and why it doesn't otherwise (a UI shows it
    /// disabled, saying so); none: always.
    pub(crate) requires: Option<(Condition, Arc<str>)>,
    /// When its row is shown; none: always.
    pub(crate) shown_when: Option<Condition>,
    /// What its row shows after it.
    pub trailing: Option<Trailing>,
    /// Whether a host may key it over time. Its kind gives the default
    /// ([`ValueKind::animatable`]); an engine turns it off for a value
    /// whose change should be one deliberate edit (a sample cap).
    pub animatable: bool,
}

impl SettingDescriptor {
    /// Setting `name`, labelled `label`, of `kind` at `default`, in group
    /// `group`: always shown and applying, without a tooltip.
    pub fn new(
        name: &str,
        label: &str,
        group: &str,
        kind: ValueKind,
        default: impl Into<Value>,
    ) -> SettingDescriptor {
        SettingDescriptor {
            name: Arc::from(name),
            label: Arc::from(label),
            group: Arc::from(group),
            tooltip: None,
            requires: None,
            shown_when: None,
            trailing: None,
            animatable: kind.animatable(),
            kind,
            default: default.into(),
        }
    }

    /// It, animatable or not (see [`animatable`](Self::animatable)).
    pub fn animatable(mut self, on: bool) -> SettingDescriptor {
        self.animatable = on;
        self
    }

    /// It, with `tooltip`.
    pub fn tooltip(mut self, tooltip: &str) -> SettingDescriptor {
        self.tooltip = Some(Arc::from(tooltip));
        self
    }

    /// It, applying while `when` holds (else disabled, saying `reason`).
    pub(crate) fn requires(mut self, when: Condition, reason: &str) -> SettingDescriptor {
        self.requires = Some((when, Arc::from(reason)));
        self
    }

    /// It, shown while `when` holds.
    pub(crate) fn shown_when(mut self, when: Condition) -> SettingDescriptor {
        self.shown_when = Some(when);
        self
    }

    /// It, with `trailing` after its row.
    pub fn trailing(mut self, trailing: Trailing) -> SettingDescriptor {
        self.trailing = Some(trailing);
        self
    }

    /// Whether its row shows while the settings hold `values`.
    pub fn is_shown(&self, values: &Parameters) -> bool {
        self.shown_when.as_ref().is_none_or(|c| c.met(values))
    }

    /// Whether it applies while the settings hold `values`.
    pub fn applies(&self, values: &Parameters) -> bool {
        self.requires.as_ref().is_none_or(|(c, _)| c.met(values))
    }

    /// Whether `value` is of its kind and within its limits.
    pub fn accepts(&self, value: &Value) -> bool {
        self.kind.check(value).is_ok()
    }
}

/// Why setting values do not fit their descriptors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingError {
    /// No descriptor has this name.
    Unknown(Arc<str>),
    /// The value is of another kind or out of range.
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

/// Every descriptor's default.
pub fn defaults(descriptors: &[SettingDescriptor]) -> Parameters {
    (descriptors.iter())
        .map(|d| (d.name.to_string(), d.default.clone()))
        .collect()
}

/// `values` made to fit `descriptors`: each descriptor's value from
/// `values` when it fits, else its default. Values no descriptor names are
/// dropped.
pub fn fit(descriptors: &[SettingDescriptor], values: &Parameters) -> Parameters {
    (descriptors.iter())
        .map(|d| {
            let v = (values.get(&*d.name))
                .filter(|v| d.accepts(v))
                .unwrap_or(&d.default);
            (d.name.to_string(), v.clone())
        })
        .collect()
}

/// Check `values` against `descriptors`: each names one and fits it.
pub fn validate(
    descriptors: &[SettingDescriptor],
    values: &Parameters,
) -> Result<(), SettingError> {
    for (name, value) in values {
        let d = (descriptors.iter())
            .find(|d| *d.name == **name)
            .ok_or_else(|| SettingError::Unknown(Arc::from(name.as_str())))?;
        if !d.accepts(value) {
            return Err(SettingError::Invalid(d.name.clone()));
        }
    }
    Ok(())
}
