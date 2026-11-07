//! [`Instancer`]: placements of other prims: real instancing. An engine
//! shares each prototype's acceleration structure and geometry between its
//! instances, whatever their number, and a move of the instances costs one
//! `TRANSFORM` on the Instancer.
//!
//! A prim an Instancer names as a prototype renders only through it: it is
//! not placed on its own, and its own placement and visibility are the
//! Instancer's business, not the scene's. A prototype is a shape or another
//! Instancer (instances of instances), and one sent before the Instancer
//! naming it, in the same batch or an earlier one.
//!
//! An instance's world placement is the Instancer's `xform` at the shutter
//! time, times the instance's own (and, nested, its prototype's). What
//! differs between instances beyond their placements is in the primvars:
//! [`Domain::Instance`] (`Cd`, `id`, `name`, `v` in scene units a second:
//! linear motion across the shutter) and [`Domain::Detail`]. An instance may
//! render with a material of its own ([`Instancer::subsets`]) in place of its
//! prototype's, or with others in place of some of its prototype's
//! ([`Instancer::material_maps`]): a file's mesh several nodes show with
//! their own materials in its slots stays one prototype.
use crate::id::{Id, PrimId};
use crate::prim::{Material, Subsets, Visibility};
use kinogaki_geometry::{Attribute, Attributes, Column, Domain, Error, Sampled, Xform};
use std::sync::Arc;

/// Placements of other prims (see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Instancer {
    prototypes: Arc<[PrimId]>,
    which: Option<Column<u32>>,
    xforms: Column<Xform>,
    primvars: Arc<Attributes>,
    subsets: Subsets,
    material_maps: MaterialMaps,
    xform: Sampled<Xform>,
    visibility: Visibility,
    path: Arc<str>,
}

/// The materials instances render with in place of some of their
/// prototypes': each instance's map, pairs of a material the shapes it
/// places name and the one it renders those primitives with. An optional
/// table of an [`Instancer`]; empty, none maps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialMaps {
    which: Column<u16>,
    maps: Arc<[Map]>,
}

/// One instance's map: pairs of the material a primitive names and the one
/// it renders with instead.
pub type Map = Arc<[(Id<Material>, Id<Material>)]>;

impl MaterialMaps {
    /// Each instance's index into `maps`, or [`Subsets::DEFAULT`] (none: its
    /// prototype's own materials).
    pub fn new(which: Column<u16>, maps: impl Into<Arc<[Map]>>) -> Self {
        Self {
            which,
            maps: maps.into(),
        }
    }

    /// Each instance's index into the maps; empty: none maps.
    pub fn which(&self) -> &Column<u16> {
        &self.which
    }

    /// The distinct maps.
    pub fn maps(&self) -> &Arc<[Map]> {
        &self.maps
    }

    /// Instance `i`'s map, when it has one.
    pub fn of(&self, i: usize) -> Option<&Map> {
        let which = *self.which.get(i)?;
        self.maps.get(usize::from(which))
    }

    fn check(&self, instances: usize) -> Result<(), Error> {
        if self.which.is_empty() {
            return Ok(());
        }
        if self.which.len() != instances {
            return Err(Error::invalid(format_args!(
                "{} material maps for {instances} instances",
                self.which.len()
            )));
        }
        let past = (self.which.iter())
            .find(|&&k| k != Subsets::DEFAULT && usize::from(k) >= self.maps.len());
        match past {
            Some(k) => Err(Error::invalid(format_args!(
                "an instance names material map {k} of {}",
                self.maps.len()
            ))),
            None => Ok(()),
        }
    }
}

impl Instancer {
    /// One instance of `prototypes` (of the first alone, until
    /// [`which`](InstancerBuilder::which) says otherwise) at each of `xforms`:
    /// no primvars, the Instancer at the origin, shown, unnamed.
    pub fn builder(
        prototypes: impl Into<Arc<[PrimId]>>,
        xforms: Column<Xform>,
    ) -> InstancerBuilder {
        InstancerBuilder(Instancer {
            prototypes: prototypes.into(),
            which: None,
            xforms,
            primvars: Arc::default(),
            subsets: Subsets::default(),
            material_maps: MaterialMaps::default(),
            xform: Sampled::still(Xform::IDENTITY),
            visibility: Visibility::SHOWN,
            path: Arc::from(""),
        })
    }

    /// A builder starting from this Instancer.
    pub fn rebuilt(&self) -> InstancerBuilder {
        InstancerBuilder(self.clone())
    }

    /// The prims it places.
    pub fn prototypes(&self) -> &Arc<[PrimId]> {
        &self.prototypes
    }

    /// Each instance's index into the prototypes; none: all the first.
    pub fn which(&self) -> Option<&Column<u32>> {
        self.which.as_ref()
    }

    /// The prototype instance `i` places.
    pub fn prototype_of(&self, i: usize) -> PrimId {
        let k = self.which.as_ref().map_or(0, |w| w[i] as usize);
        self.prototypes[k]
    }

    /// Each instance's placement, relative to the Instancer. Shared: compare
    /// by [`Column::ptr_eq`] to see whether they moved.
    pub fn xforms(&self) -> &Column<Xform> {
        &self.xforms
    }

    /// What differs between instances: [`Domain::Instance`] and
    /// [`Domain::Detail`] primvars.
    pub fn primvars(&self) -> &Arc<Attributes> {
        &self.primvars
    }

    /// Instance `i`'s primvar `name`: its own ([`Domain::Instance`]), else
    /// the Instancer's ([`Domain::Detail`]); the attribute and the element
    /// holding it.
    pub fn primvar(&self, i: usize, name: &str) -> Option<(&Attribute, usize)> {
        let own = self.primvars.get(Domain::Instance, name).map(|a| (a, i));
        own.or_else(|| self.primvars.get(Domain::Detail, name).map(|a| (a, 0)))
    }

    /// The material each instance renders with in place of its prototype's
    /// (none named, or none: its prototype's).
    pub fn subsets(&self) -> &Subsets {
        &self.subsets
    }

    /// The materials each instance renders with in place of some of its
    /// prototype's, where it has no material of its own (`subsets`).
    pub fn material_maps(&self) -> &MaterialMaps {
        &self.material_maps
    }

    /// The material instance `i` renders a primitive of the shapes it places
    /// with, the primitive naming `own`: the instance's own material, else
    /// its map's in place of `own`, else `own`.
    pub fn material(&self, i: usize, own: Option<Id<Material>>) -> Option<Id<Material>> {
        if let Some(whole) = self.subsets.of(i) {
            return Some(whole);
        }
        let mapped = (self.material_maps.of(i)).and_then(|map| {
            let own = own?;
            map.iter().find(|(from, _)| *from == own).map(|&(_, to)| to)
        });
        mapped.or(own)
    }

    /// The Instancer to world across the shutter.
    pub fn xform(&self) -> &Sampled<Xform> {
        &self.xform
    }

    /// Whether its instances render, and what sees them.
    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    /// The host's path of it: the name Cryptomatte gives its instances
    /// (with `name`, when they have one).
    pub fn path(&self) -> &Arc<str> {
        &self.path
    }
}

/// Makes an [`Instancer`], checking it whole: an index within the prototypes
/// for every instance, primvars of one value an element of their domain.
#[derive(Clone, Debug)]
pub struct InstancerBuilder(Instancer);

impl InstancerBuilder {
    /// With `prototypes` in place of the prims it places.
    pub fn prototypes(mut self, prototypes: impl Into<Arc<[PrimId]>>) -> Self {
        self.0.prototypes = prototypes.into();
        self
    }

    /// With each instance's index into the prototypes.
    pub fn which(mut self, which: Option<Column<u32>>) -> Self {
        self.0.which = which;
        self
    }

    /// With `xforms` in place of its instances' placements.
    pub fn xforms(mut self, xforms: Column<Xform>) -> Self {
        self.0.xforms = xforms;
        self
    }

    /// With `primvars` in place of all it holds.
    pub fn primvars(mut self, primvars: impl Into<Arc<Attributes>>) -> Self {
        self.0.primvars = primvars.into();
        self
    }

    /// With each instance's material as `subsets` says.
    pub fn subsets(mut self, subsets: Subsets) -> Self {
        self.0.subsets = subsets;
        self
    }

    /// With each instance's materials in place of its prototype's as `maps`
    /// says.
    pub fn material_maps(mut self, maps: MaterialMaps) -> Self {
        self.0.material_maps = maps;
        self
    }

    /// With `xform` across the shutter.
    pub fn xform(mut self, xform: Sampled<Xform>) -> Self {
        self.0.xform = xform;
        self
    }

    /// With `visibility`.
    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.0.visibility = visibility;
        self
    }

    /// Named `path`.
    pub fn path(mut self, path: impl Into<Arc<str>>) -> Self {
        self.0.path = path.into();
        self
    }

    /// The Instancer.
    ///
    /// # Errors
    /// The first rule broken (see the type's docs).
    pub fn build(self) -> Result<Instancer, Error> {
        let i = self.0;
        let instances = i.xforms.len();
        if instances > 0 && i.prototypes.is_empty() {
            return Err(Error::invalid("instances of no prototype"));
        }
        if let Some(which) = &i.which {
            if which.len() != instances {
                return Err(Error::invalid(format_args!(
                    "{} prototype indices for {instances} instances",
                    which.len()
                )));
            }
            let top = which.derived(|w| Highest(w.iter().copied().max()));
            if top.0.is_some_and(|k| k as usize >= i.prototypes.len()) {
                return Err(Error::invalid(format_args!(
                    "an instance names a prototype past the {}",
                    i.prototypes.len()
                )));
            }
        }
        i.subsets.check(instances)?;
        i.material_maps.check(instances)?;
        i.primvars
            .check("instancers", None, |domain| match domain {
                Domain::Instance => Some(instances),
                Domain::Detail => Some(1),
                _ => None,
            })?;
        Ok(i)
    }
}

/// The highest index of a column, kept on it.
struct Highest(Option<u32>);
