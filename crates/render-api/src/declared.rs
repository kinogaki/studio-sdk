//! Prim types an engine declares: lights and atmospheres a host places in
//! its scene, each a [`Schema`] of parameters. A host keeps such a prim's
//! values by name and draws its rows from the schema, without knowing the
//! type; only engines that declare a type render it.
//!
//! The standard types ([`crate::standard`]: UsdLux's lights and fog) are
//! declared alike by every engine that renders them, so they keep their id
//! across engines. An engine adds types of its own (a rim light).

use kinogaki_geometry::{Sampled, Xform};
use std::sync::Arc;

use crate::settings::Textures;
use kinogaki_value::{Parameters, ParametersExt, PrimTypeId, Schema, Value};

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
    /// Its id (`acme.ray.rect_light`, `kinogaki.pixel.rim_light`).
    pub id: PrimTypeId,
    /// What a UI calls it (`Rect Light`).
    pub label: Arc<str>,
    /// Its role.
    pub role: Role,
    /// Its values.
    schema: Schema,
}

impl PrimType {
    /// Type `id` (under the declaring package's vendor: the registry
    /// refuses another's, and an own type two packages declare) that a UI
    /// calls `label`, of `role`, its values `schema`'s: an engine's own type
    /// (a rim light).
    pub fn new(id: PrimTypeId, label: &str, role: Role, schema: Schema) -> PrimType {
        PrimType {
            id,
            label: Arc::from(label),
            role,
            schema,
        }
    }

    /// Its values' schema: what a prim of it holds by name, in the sections a
    /// UI shows them in.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// It with `more`'s sections and values after its own
    /// ([`Schema::merged`]): an engine's own section on a standard type (a
    /// gobo on a spot light).
    pub fn merged(mut self, more: Schema) -> PrimType {
        self.schema = self.schema.merged(more);
        self
    }
}

/// A placed prim of a declared type: its values by parameter name and where
/// it sits. A host sends the prims that render: a hidden light is no prim.
#[derive(Clone, Debug, PartialEq)]
pub struct Declared {
    /// Its type's id.
    pub type_id: PrimTypeId,
    /// Its values, by parameter name. A host sends every parameter the
    /// engine declares for the type; one missing takes its default. An
    /// image value is its file's path; an engine reads its texture.
    pub values: Parameters,
    /// Its image values' Texture prims (filled by the host as it sends the
    /// prim).
    pub textures: Textures,
    /// Prim to world across the shutter (unused by atmospheres).
    pub xform: Sampled<Xform>,
}

impl Declared {
    /// A prim of type `type_id` holding `values`, at the origin.
    pub fn new(type_id: PrimTypeId, values: Parameters) -> Declared {
        Declared {
            type_id,
            values,
            textures: Textures::new(),
            xform: Sampled::still(Xform::IDENTITY),
        }
    }

    /// Value `name`.
    pub fn value(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    // An engine reads its values through `ParametersExt` on `values`; these
    // say a default on a miss, which a host's prim never has (it sends every
    // value the engine declares, within its limits).

    /// Number `name`, as `f32`; 0 on a miss.
    pub fn float(&self, name: &str) -> f32 {
        self.values.number(name).unwrap_or(0.0) as f32
    }

    /// Toggle `name`; off on a miss.
    pub fn bool(&self, name: &str) -> bool {
        self.values.toggle(name).unwrap_or(false)
    }

    /// Choice `name`'s key; empty on a miss.
    pub fn choice(&self, name: &str) -> &str {
        self.values.choice(name).unwrap_or("")
    }

    /// Color `name`, as `f32`; black on a miss.
    pub fn color(&self, name: &str) -> [f32; 3] {
        self.values
            .color(name)
            .unwrap_or([0.0; 3])
            .map(|c| c as f32)
    }
}
