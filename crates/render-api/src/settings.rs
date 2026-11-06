//! Settings: what every view shares ([`SceneSettings`]) and the
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

use kinogaki_geometry::{Error, Rate, Units};
use kinogaki_value::{Condition, NumberLook, Parameters, Value, ValueKind};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::geometry::image::Image;
use crate::id::Id;
use crate::prim::Camera;

/// The Texture prims the host decoded image values to, by the value's
/// name: what an engine reads for each [`ValueKind::File`] value (one
/// missing: no image, or one not read yet).
pub type Textures = BTreeMap<String, Id<Image>>;

/// What every view of an engine shares. What differs between views travels
/// with the view ([`crate::ViewDesc::values`]).
#[derive(Clone, Debug, PartialEq)]
pub struct SceneSettings {
    /// The Camera prim whose shutter placements blur across (none: nothing
    /// blurs).
    pub shutter: Option<Id<Camera>>,
    /// The scene's units: what real sizes (a lens's millimetres, an
    /// anodized part's grain) are measured against, and the axis that is
    /// up to sky light and height fog.
    pub units: Units,
    /// The scene's frames a second: how long a frame is, which sample times
    /// (in frames) and velocities (in scene units a second) meet at.
    pub rate: Rate,
}

impl Default for SceneSettings {
    fn default() -> SceneSettings {
        SceneSettings {
            shutter: None,
            units: Units::METERS,
            rate: Rate::FILM,
        }
    }
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

    /// It, open whatever [`open`](Self::open) says while `when` holds.
    pub fn open_when(mut self, when: Condition) -> SettingGroup {
        self.open_when = Some(when);
        self
    }

    /// Whether it starts open, its settings holding `values`.
    pub fn is_open(&self, values: &Parameters) -> bool {
        self.open || self.open_when.as_ref().is_some_and(|c| c.met(values))
    }
}

/// A setting an engine publishes, so a UI can show it without knowing it.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingDescriptor {
    /// The key in [`crate::ViewDesc::values`] and in files.
    pub name: Arc<str>,
    /// What a UI labels it.
    pub label: Arc<str>,
    /// What it holds.
    pub kind: ValueKind,
    /// How its numbers show and move; the host's defaults for their unit
    /// unless set.
    pub look: NumberLook,
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
            animatable: kind.animatable(),
            kind,
            look: NumberLook::default(),
            default: default.into(),
        }
    }

    /// It, animatable or not (see [`animatable`](Self::animatable)).
    pub fn animatable(mut self, on: bool) -> SettingDescriptor {
        self.animatable = on;
        self
    }

    /// It, its numbers shown and moved as `look` says.
    pub fn look(mut self, look: NumberLook) -> SettingDescriptor {
        self.look = look;
        self
    }

    /// It, with `tooltip`.
    pub fn tooltip(mut self, tooltip: &str) -> SettingDescriptor {
        self.tooltip = Some(Arc::from(tooltip));
        self
    }

    /// It, applying while `when` holds (else disabled, saying `reason`).
    pub fn requires(mut self, when: Condition, reason: &str) -> SettingDescriptor {
        self.requires = Some((when, Arc::from(reason)));
        self
    }

    /// It, shown while `when` holds.
    pub fn shown_when(mut self, when: Condition) -> SettingDescriptor {
        self.shown_when = Some(when);
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
        self.kind.check(value, &self.label).is_ok()
    }
}

/// The names of the settings a host's render output holds of its own beside
/// its engine's: the image's size, the camera, the output file and the motion
/// samples. An engine names none of its settings so (the registry refuses
/// one that does).
pub const OUTPUT_SETTINGS: [&str; 4] = ["resolution", "camera", "output", "motionSamples"];

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
pub fn validate(descriptors: &[SettingDescriptor], values: &Parameters) -> Result<(), Error> {
    for (name, value) in values {
        let d = (descriptors.iter())
            .find(|d| *d.name == **name)
            .ok_or_else(|| Error::invalid(format_args!("unknown setting {name:?}")))?;
        if !d.accepts(value) {
            let name = &d.name;
            return Err(Error::invalid(format_args!(
                "setting {name:?} is out of range"
            )));
        }
    }
    Ok(())
}
