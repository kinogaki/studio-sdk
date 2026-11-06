//! [`DeclaredKind`]: an engine's own object kind whose objects are prims of
//! the types it declares: its lights (placed, one kind whatever the light
//! is) or its atmosphere (fog, at the root). Each engine brings its own
//! (`acme.ray.light`), so a scene's lights belong to the engine that made
//! them; core syncs them through [`RenderedKind`] to whichever engine
//! renders, filled out with that engine's schema.
//!
//! An object holds one prim: its type (one of the kind's) and every value it
//! holds, written back as read. A light of a standard type gives its light as
//! a standard light value (where it is is the object's), which formats carry,
//! and an imported light value is adopted as a light of its standard type.
//! How lights show in the viewport is `view`'s.
//!
//! ```text
//! def "acme.ray.light" "body" {   … type (its prim type's id)
//!     def values "values" { … }   every value it holds, by name
//! }
//! ```
use super::view as light_view;
use crate::cook::Handle;
use crate::element::{self, ElementReader, ElementWriter, Field};
use crate::facets::Glyphs;
use crate::{
    Adoption, ContextId, CreateCx, CreationId, Data, DataTypeId, Diagnostic, Error, KindCreation,
    ObjectBody, ObjectCodec, ObjectKindId, ObjectKindSpec, Parameters, ReadCx, RenderedKind, Value,
    Valued, standard as data,
};
use kinogaki_render_api::standard::{self, Shape};
use kinogaki_render_api::{Declared, PrimType, Role};
use std::sync::Arc;

/// The element holding a prim's values.
const VALUES: &str = "values";

/// Where menus list lights (after Camera, before Render Output) and
/// atmospheres (after Render Output).
const LIGHTS_ORDER: i32 = -600;
const ATMOSPHERE_ORDER: i32 = -400;

/// An object of a [`DeclaredKind`]: its prim, unplaced.
#[derive(Debug)]
pub(super) struct DeclaredBody {
    pub(super) kind: ObjectKindId,
    pub(super) prim: Declared,
}

impl ObjectBody for DeclaredBody {
    fn kind(&self) -> ObjectKindId {
        self.kind
    }
}

/// An engine's kind of objects that are prims of its declared types (see
/// the module docs).
pub struct DeclaredKind {
    id: ObjectKindId,
    label: String,
    icon: &'static str,
    family: String,
    role: Role,
    types: Vec<PrimType>,
}

/// Whether every number in `values` is finite.
fn finite(values: &Parameters) -> bool {
    values.values().all(|value| match value {
        Value::Number(x) => x.is_finite(),
        Value::Numbers(n) => n.iter().all(|x| x.is_finite()),
        Value::Color(c) => c.iter().all(|x| x.is_finite()),
        _ => true,
    })
}

/// Words other applications use for a light of a standard shape.
fn aliases(ty: &PrimType) -> &'static [&'static str] {
    let prim = Declared::new(ty.id, ty.defaults());
    match standard::Light::decode(&prim).map(|light| light.shape) {
        Some(Shape::Rect { .. } | Shape::Disk { .. }) => {
            &["usdlux", "lamp", "rect", "disk", "rectlight", "disklight"]
        }
        Some(Shape::Dome { .. }) => &["usdlux", "environment", "sky", "domelight", "hdri"],
        Some(Shape::Distant { .. }) => &["usdlux", "sun", "directional", "distantlight"],
        Some(Shape::Spot { .. }) => &["usdlux", "spotlight", "sphere", "spherelight", "cone"],
        None if ty.role == Role::Atmosphere => &["mist", "haze", "atmosphere", "height fog"],
        None => &[],
    }
}

impl DeclaredKind {
    /// An engine's lights: kind `id`, placed objects of one of `types` (its
    /// light-role types; others are left out), listed under `family`. A new
    /// one is of the first type at its defaults.
    pub fn lights(id: ObjectKindId, family: &str, types: Vec<PrimType>) -> Self {
        Self {
            id,
            label: "Light".into(),
            icon: "light-area",
            family: family.into(),
            role: Role::Light,
            types: types
                .into_iter()
                .filter(|t| t.role == Role::Light)
                .collect(),
        }
    }

    /// An engine's atmosphere of type `ty` (its fog): kind `id`, objects at
    /// the scene's root, named as the type is.
    pub fn atmosphere(id: ObjectKindId, ty: PrimType) -> Self {
        Self {
            id,
            label: ty.label.to_string(),
            icon: "ghosted",
            family: String::new(),
            role: Role::Atmosphere,
            types: vec![ty],
        }
    }

    /// Its type of id `id` (as stored).
    fn type_of(&self, id: &str) -> Option<&PrimType> {
        self.types.iter().find(|t| t.id.as_str() == id)
    }

    /// The creation making an object of type `ty`: under the kind's own
    /// prefix (`acme.ray.light` makes `acme.ray.area_light`), named by the
    /// type's last step.
    fn creation_of(&self, ty: &PrimType) -> Option<CreationId> {
        let prefix = self.id.as_str().rsplit_once('.')?.0;
        let name = ty.id.name();
        CreationId::parse(&format!("{prefix}.{name}"))
    }

    /// A body of `prim`, when it fits the kind: one of its types, every value
    /// its type declares within its limits, every number finite.
    fn body(&self, prim: &Declared) -> Option<Arc<dyn ObjectBody>> {
        let ty = self.type_of(prim.type_id.as_str())?;
        let fits =
            (ty.params.iter()).all(|d| prim.value(&d.name).is_none_or(|value| d.accepts(value)));
        (fits && finite(&prim.values)).then(|| {
            let prim = Declared::new(ty.id, prim.values.clone());
            Arc::new(DeclaredBody {
                kind: self.id,
                prim,
            }) as Arc<dyn ObjectBody>
        })
    }

    /// `body`'s prim, when it is one of this kind's.
    fn prim<'a>(&self, body: &'a dyn ObjectBody) -> Option<&'a Declared> {
        let body = body.downcast_ref::<DeclaredBody>()?;
        (body.kind == self.id).then_some(&body.prim)
    }

    /// Whether its objects are lights.
    fn of_lights(&self) -> bool {
        self.role == Role::Light
    }
}

impl ObjectKindSpec for DeclaredKind {
    fn id(&self) -> ObjectKindId {
        self.id
    }
    fn label(&self) -> &str {
        &self.label
    }
    fn icon(&self) -> &str {
        self.icon
    }
    fn placed(&self) -> bool {
        self.of_lights()
    }
    fn context(&self) -> Option<ContextId> {
        None
    }
    /// A light's value: a standard light.
    fn result(&self) -> Option<DataTypeId> {
        self.of_lights().then(|| data::light().id())
    }
    fn new_body(&self) -> Arc<dyn ObjectBody> {
        let prim = match self.types.first() {
            Some(ty) => Declared::new(ty.id, ty.defaults()),
            None => standard::Light::default().declared(),
        };
        Arc::new(DeclaredBody {
            kind: self.id,
            prim,
        })
    }
    fn codec(&self) -> &dyn ObjectCodec {
        self
    }
    fn adopted(&self) -> Vec<DataTypeId> {
        match self.of_lights() {
            true => vec![data::light().id()],
            false => Vec::new(),
        }
    }
    /// An imported light value: a light of its standard type, when the kind
    /// has it.
    fn adopts(&self, value: &Data, _: &CreateCx<'_>) -> Option<Adoption> {
        let light = data::light().get(value).filter(|_| self.of_lights())?;
        self.body(&light.declared()).map(Adoption::new)
    }
    /// A light of a standard type as a light value, saying what it leaves
    /// out: the values no standard light type has (the engine's own). None
    /// for a light no light value is ([`light_value`](super::light_value)).
    fn value(&self, body: &dyn ObjectBody, _path: &str) -> Option<Valued> {
        let prim = self.prim(body).filter(|_| self.of_lights())?;
        let light = super::light_value(standard::Light::decode(prim)?).ok()?;
        let types = standard::lights();
        let own =
            |name: &String| (types.iter()).all(|t| t.params.iter().all(|d| *d.name != **name));
        let warnings = (prim.values.keys().filter(|name| own(name)))
            .map(|name| Diagnostic::warning(format!("{name} isn't a standard light's; left out")))
            .collect();
        Some(Valued {
            value: data::light().wrap(light),
            warnings,
        })
    }
    fn at_root(&self) -> bool {
        !self.of_lights()
    }
    fn renders(&self) -> Option<&dyn RenderedKind> {
        Some(self)
    }
    fn glyphs(&self) -> Option<&dyn Glyphs> {
        self.of_lights()
            .then_some(&light_view::LightGlyphs as &dyn Glyphs)
    }
    fn handles(&self, body: &dyn ObjectBody) -> Vec<Handle> {
        (self.prim(body)).map_or_else(Vec::new, light_view::handles)
    }
    fn creations(&self, _: &CreateCx<'_>) -> Vec<KindCreation> {
        (self.types.iter())
            .filter_map(|ty| {
                let mut made = KindCreation::new(self.creation_of(ty)?, &*ty.label);
                made.family = self.family.clone();
                made.aliases = aliases(ty).iter().map(|&a| a.to_owned()).collect();
                made.order = if self.of_lights() {
                    LIGHTS_ORDER
                } else {
                    ATMOSPHERE_ORDER
                };
                Some(made)
            })
            .collect()
    }
    fn create(&self, id: CreationId, _: &CreateCx<'_>) -> Result<Arc<dyn ObjectBody>, Error> {
        let ty = (self.types.iter())
            .find(|ty| self.creation_of(ty) == Some(id))
            .ok_or_else(|| Error::invalid(format_args!("{} makes no {id}", self.label)))?;
        let prim = Declared::new(ty.id, ty.defaults());
        Ok(Arc::new(DeclaredBody {
            kind: self.id,
            prim,
        }))
    }
}

impl RenderedKind for DeclaredKind {
    fn declared(&self, body: &dyn ObjectBody) -> Vec<Declared> {
        self.prim(body)
            .map(|prim| vec![prim.clone()])
            .unwrap_or_default()
    }
    fn with_declared(
        &self,
        body: &dyn ObjectBody,
        declared: &[Declared],
    ) -> Option<Arc<dyn ObjectBody>> {
        self.prim(body)?;
        match declared {
            [prim] => self.body(prim),
            _ => None,
        }
    }
}

/// An object saves its type and every value it holds.
impl ObjectCodec for DeclaredKind {
    fn write(&self, body: &dyn ObjectBody, out: &mut dyn ElementWriter) -> Result<(), Error> {
        let prim = self
            .prim(body)
            .ok_or_else(|| Error::invalid("not this kind's"))?;
        out.set("type", Field::Text(prim.type_id.to_string()))?;
        element::write_values(&mut *out.child(VALUES, VALUES)?, &prim.values)
    }
    fn read(&self, at: &dyn ElementReader, _: &ReadCx<'_>) -> Result<Arc<dyn ObjectBody>, Error> {
        let id = at.text("type")?;
        let ty = self.type_of(id).ok_or_else(|| {
            Error::invalid(format_args!(
                "{}: {} has no type {id}",
                at.location(),
                self.label
            ))
        })?;
        let values = element::read_values(&*at.required_child(VALUES)?, &ty.params)?;
        self.body(&Declared::new(ty.id, values)).ok_or_else(|| {
            Error::invalid(format_args!(
                "{}: a value doesn't fit its type",
                at.location()
            ))
        })
    }
}

#[cfg(test)]
mod tests;
