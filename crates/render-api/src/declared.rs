//! Prim types an engine declares: lights and atmospheres a host places in
//! its scene, each a parameter schema of [`SettingDescriptor`]s. A host
//! keeps such a prim's values by name and draws its rows from the schema,
//! without knowing the type; only engines that declare a type render it.
//!
//! The standard types ([`crate::standard`]: UsdLux's lights and fog) are
//! declared alike by every engine that renders them, so they keep their id
//! across engines. An engine adds types of its own (a rim light).

use kinogaki_geometry::{Sampled, Xform};
use std::sync::Arc;

use crate::prim::Visibility;
use crate::settings::{SettingDescriptor, SettingGroup, Textures, defaults};
use kinogaki_value::{Parameters, Value};

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
    pub(crate) groups: Vec<SettingGroup>,
    /// Its parameters, in the order a UI lists them.
    pub params: Vec<SettingDescriptor>,
}

impl PrimType {
    /// Type `id` (under the declaring package's vendor) that a UI calls
    /// `label`, of `role`, with no parameter yet: an engine's own type (a rim
    /// light).
    pub fn new(id: &str, label: &str, role: Role) -> PrimType {
        PrimType {
            id: Arc::from(id),
            label: Arc::from(label),
            role,
            groups: Vec::new(),
            params: Vec::new(),
        }
    }

    /// It with `group` and its parameters `params` after what it has: an
    /// engine's own section on a standard type (a gobo on a spot light).
    pub fn group(mut self, group: SettingGroup, params: Vec<SettingDescriptor>) -> PrimType {
        self.groups.push(group);
        self.params.extend(params);
        self
    }

    /// Every parameter's default.
    pub fn defaults(&self) -> Parameters {
        defaults(&self.params)
    }
}

/// A placed prim of a declared type: its values by parameter name, where it sits and whether it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Declared {
    /// Its type's id.
    pub type_id: Arc<str>,
    /// Its values, by parameter name. A host sends every parameter the
    /// engine declares for the type; one missing takes its default. An
    /// image value is its file's path; an engine reads its texture.
    pub values: Parameters,
    /// Its image values' Texture prims (filled by the host as it sends the
    /// prim).
    pub textures: Textures,
    /// Prim to world across the shutter (unused by atmospheres).
    pub xform: Sampled<Xform>,
    /// Whether it is in the render, and whether the camera sees it itself (a
    /// light lights the scene either way).
    pub visibility: Visibility,
}

impl Declared {
    /// A prim of type `type_id` holding `values`, at the origin, visible.
    pub fn new(type_id: &str, values: Parameters) -> Declared {
        Declared {
            type_id: Arc::from(type_id),
            values,
            textures: Textures::new(),
            xform: Sampled::still(Xform::IDENTITY),
            visibility: Visibility::SHOWN,
        }
    }

    /// Value `name`.
    pub fn value(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// Number `name`, as `f32` (0 when missing or not a number).
    pub fn float(&self, name: &str) -> f32 {
        self.value(name).and_then(Value::number).unwrap_or(0.0) as f32
    }

    /// Toggle `name` (off when missing).
    pub fn bool(&self, name: &str) -> bool {
        self.value(name).and_then(Value::bool).unwrap_or(false)
    }

    /// Choice `name`'s key (empty when missing).
    pub fn choice(&self, name: &str) -> &str {
        self.value(name).and_then(Value::choice).unwrap_or("")
    }

    /// Color `name`, as `f32` (black when missing).
    pub fn color(&self, name: &str) -> [f32; 3] {
        self.value(name)
            .and_then(Value::color)
            .unwrap_or([0.0; 3])
            .map(|c| c as f32)
    }
}
