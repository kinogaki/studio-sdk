//! Prim types an engine declares: lights and atmospheres a host places in
//! its scene, each a [`Schema`] of parameters. A host keeps such a prim's
//! values by name and draws its rows from the schema, without knowing the
//! type; only engines that declare a type render it.
//!
//! The standard types ([`crate::standard`]: UsdLux's lights and fog) are
//! declared alike by every engine that renders them, so they keep their id
//! across engines. An engine adds types of its own (a rim light).
//!
//! A prim's object rows ([`ValueKind::Object`]: a portal's dome, a geometry
//! light's mesh) name the host's objects, which an engine never sees: the
//! host sends what each names as the prims it renders as
//! ([`Declared::members`]).
//!
//! [`ValueKind::Object`]: kinogaki_value::ValueKind::Object

use kinogaki_geometry::{Sampled, Xform};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::id::PrimId;
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

/// The prims an object row names, as the host resolved it for one engine:
/// an object's Shape prims and Instancers (a prototype renders through the
/// Instancers placing it) and the prims its kind declares. Held sorted, so
/// [`holds`](Self::holds) searches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Members(Arc<[PrimId]>);

impl Members {
    /// `prims`, sorted, each once.
    pub fn new(prims: impl IntoIterator<Item = PrimId>) -> Members {
        let mut prims: Vec<PrimId> = prims.into_iter().collect();
        prims.sort_unstable();
        prims.dedup();
        Members(prims.into())
    }

    /// Whether it holds `prim`.
    pub fn holds(&self, prim: PrimId) -> bool {
        self.0.binary_search(&prim).is_ok()
    }

    /// The prims it holds, in order.
    pub fn prims(&self) -> &[PrimId] {
        &self.0
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
    /// What its object rows name, by the row's name, as prims of this
    /// engine's scene (filled by the host as it sends the prim, and sent
    /// again when they change); a row naming nothing, or no object row, has
    /// none.
    pub members: BTreeMap<String, Members>,
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
            members: BTreeMap::new(),
            xform: Sampled::still(Xform::IDENTITY),
        }
    }

    /// What object row `name` names (none when it names nothing).
    pub fn members(&self, name: &str) -> Members {
        self.members.get(name).cloned().unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn prim(slot: u32) -> PrimId {
        PrimId {
            slot,
            generation: 0,
        }
    }

    #[test]
    fn members_hold_the_prims_named_each_once() {
        let named = Members::new([prim(3), prim(1), prim(3)]);
        assert_eq!(named.prims(), [prim(1), prim(3)]);
        assert!(named.holds(prim(1)) && !named.holds(prim(2)));
        assert!(!Members::default().holds(prim(0)));
    }
}
