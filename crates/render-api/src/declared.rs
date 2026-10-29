//! Prim types an engine declares: lights and atmospheres a host places in
//! its scene, each a parameter schema of [`SettingDescriptor`]s. A host
//! keeps such a prim's values by name and draws its rows from the schema,
//! without knowing the type; only engines that declare a type render it.
//!
//! The standard types ([`crate::standard`]: UsdLux's lights and fog) are
//! declared alike by every engine that renders them, so they keep their id
//! across engines. An engine adds types of its own (Pixel's rim light) and
//! parameters of its own on others' types ([`AppliedSchema`], like USD's
//! API schemas: Pixel's gobo on distant lights and spots).

use std::sync::Arc;

use crate::prim::{IDENTITY, Matrix};
use crate::settings::{
    SettingDescriptor, SettingGroup, SettingValue, SettingValues, defaults, value,
};

/// What a declared type is to a host: where it sits and how a viewport
/// shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// A light, placed by its transform.
    Light,
    /// Something filling the scene (fog), with no place of its own; a
    /// scene has at most one of each type.
    Atmosphere,
}

/// A prim type an engine declares.
#[derive(Clone, Debug, PartialEq)]
pub struct PrimType {
    /// Its id (`kinogaki.render.area_light`, `kinogaki.pixel.rim_light`).
    pub id: Arc<str>,
    /// What a UI calls it (`Area Light`).
    pub label: Arc<str>,
    /// Its role.
    pub role: Role,
    /// Its parameters' groups, in order.
    pub groups: Vec<SettingGroup>,
    /// Its parameters, in the order a UI lists them.
    pub params: Vec<SettingDescriptor>,
}

impl PrimType {
    /// Its parameter `name`.
    pub fn param(&self, name: &str) -> Option<&SettingDescriptor> {
        self.params.iter().find(|d| &*d.name == name)
    }

    /// Every parameter's default.
    pub fn defaults(&self) -> SettingValues {
        defaults(&self.params)
    }
}

/// Parameters an engine adds to other types (USD's applied API schemas):
/// shown and rendered where an engine declaring it renders the type, kept
/// either way.
#[derive(Clone, Debug, PartialEq)]
pub struct AppliedSchema {
    /// Its id (`kinogaki.pixel.gobo`).
    pub id: Arc<str>,
    /// The types it applies to, by id.
    pub applies_to: Vec<Arc<str>>,
    /// Its parameters' groups, in order (after the type's).
    pub groups: Vec<SettingGroup>,
    /// Its parameters (names no type it applies to uses).
    pub params: Vec<SettingDescriptor>,
}

impl AppliedSchema {
    /// Whether it applies to type `id`.
    pub fn applies(&self, id: &str) -> bool {
        self.applies_to.iter().any(|t| &**t == id)
    }
}

/// A placed prim of a declared type: its values by parameter name (the
/// type's and its applied schemas'), where it sits and whether it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Declared {
    /// Its type's id.
    pub type_id: Arc<str>,
    /// Its values, by parameter name. A host sends every parameter the
    /// engine declares for the type; one missing takes its default.
    pub values: SettingValues,
    /// Prim to world (unused by atmospheres).
    pub transform: Matrix,
    /// Whether the camera sees it itself (a light lights the scene either
    /// way).
    pub visible: bool,
}

impl Declared {
    /// A prim of type `type_id` holding `values`, at the origin, visible.
    pub fn new(type_id: &str, values: SettingValues) -> Declared {
        Declared {
            type_id: Arc::from(type_id),
            values,
            transform: IDENTITY,
            visible: true,
        }
    }

    /// Value `name`.
    pub fn value(&self, name: &str) -> Option<&SettingValue> {
        value(&self.values, name)
    }

    /// Made type `ty`: values of the same names are kept (so switching
    /// back restores them), and the type's others, or those that don't
    /// fit it, take their defaults.
    pub fn retype(&mut self, ty: &PrimType) {
        self.type_id = ty.id.clone();
        for d in &ty.params {
            let fits = value(&self.values, &d.name).is_some_and(|v| d.accepts(v));
            if !fits {
                crate::settings::set_value(&mut self.values, &d.name, d.default_value());
            }
        }
    }

    /// Number `name`, as `f32` (0 when missing or not a number).
    pub fn float(&self, name: &str) -> f32 {
        self.value(name)
            .and_then(SettingValue::as_float)
            .unwrap_or(0.0) as f32
    }

    /// Toggle `name` (off when missing).
    pub fn bool(&self, name: &str) -> bool {
        self.value(name)
            .and_then(SettingValue::as_bool)
            .unwrap_or(false)
    }

    /// Choice `name`'s name (empty when missing).
    pub fn choice(&self, name: &str) -> &str {
        self.value(name)
            .and_then(SettingValue::as_choice)
            .unwrap_or("")
    }

    /// Color `name`, as `f32` (black when missing).
    pub fn color(&self, name: &str) -> [f32; 3] {
        self.value(name)
            .and_then(SettingValue::as_color)
            .unwrap_or([0.0; 3])
            .map(|c| c as f32)
    }
}
