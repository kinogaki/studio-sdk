//! Admission of where a package lists what it offers ([`menu`](crate::menu)):
//! every home is one of each place's topics or the package's family.
use super::Registry;
use crate::Error;
use crate::cook::{Description, Reach};
use crate::id::{ContextId, NodeTypeId, PackageId};
use crate::menu::{Home, topics};
use crate::network::NetworkContext;
use crate::object::{CreateCx, KindCreation};
use crate::package::Package;
use kinogaki_render_api::{EngineDescription, Engines};

/// Core's package, which lists everything under topics: it has no family.
const CORE: &str = "kinogaki.core";

/// A place an offering is listed at.
#[derive(Clone, Copy)]
enum Place<'a> {
    /// The object levels (the scene root, a Part).
    Objects,
    /// Networks of this context.
    Network(&'a NetworkContext),
    /// Every network: only what every one has (Import) or a family fits.
    Everywhere,
}

impl Place<'_> {
    /// Whether it has `topic`.
    fn has(self, topic: crate::menu::TopicId) -> bool {
        match self {
            Place::Objects => topics::objects::all().iter().any(|t| t.id == topic),
            Place::Network(c) => c.listed_topics().iter().any(|t| t.id == topic),
            Place::Everywhere => topic == topics::IMPORT,
        }
    }

    /// What a refusal calls it.
    fn name(self) -> String {
        match self {
            Place::Objects => "the object levels".into(),
            Place::Network(c) => format!("{}'s networks", c.id),
            Place::Everywhere => "every network".into(),
        }
    }
}

impl Registry {
    /// Every home `package` names fits each place it is listed at: its
    /// topic is the place's (core's for the object levels and every
    /// network's Import, a context's own for its networks), or it is the
    /// package's family, which core's package has none of; no group is
    /// blank; an item of a library that names a node type takes that type's
    /// home (listed nowhere when the type isn't), any other names its own;
    /// and a package with a family is labelled as no other one with a
    /// family is.
    pub(super) fn check_homes(&self, package: &Package) -> Result<(), Error> {
        let id = package.id;
        let mut family = false;
        let mut fits = |what: &str, home: &Home, places: &[Place<'_>]| {
            fits(id, what, home, places)?;
            family |= home.topic.is_none();
            Ok::<(), Error>(())
        };
        for t in &package.node_types {
            let d = t.description();
            if let Some(listing) = &d.listing {
                let what = format!("node type {}", t.id());
                fits(&what, &listing.home, &self.offered(package, &d))?;
            }
        }
        for c in &package.contexts {
            for a in &c.adopts {
                let what = format!("{}'s adoption of {}", c.id, a.node_type);
                fits(&what, &a.home, &[Place::Network(c)])?;
            }
        }
        for c in &package.creations {
            let ty = c.node.node_type;
            let mut places = match self.description(package, ty) {
                Some(d) => self.offered(package, &d),
                None => Vec::new(),
            };
            // Listed where its type is adopted too.
            places.extend(
                (package.contexts.iter())
                    .chain(self.contexts.values().map(|e| &*e.item))
                    .filter(|n| n.adopts.iter().any(|a| a.node_type == ty))
                    .map(Place::Network),
            );
            fits(&format!("node creation {}", c.id), &c.home, &places)?;
        }
        for c in self.object_creations(package)? {
            fits(
                &format!("object creation {}", c.id),
                &c.home,
                &[Place::Objects],
            )?;
        }
        for item in package.ui.iter().flat_map(|ui| ui.items()) {
            let what = format!("{}'s item {}", item.context, item.item);
            let named = NodeTypeId::parse(&item.item).and_then(|n| self.description(package, n));
            let home = match (&item.home, named) {
                (Some(_), Some(_)) => {
                    return Err(Error::refused(format!(
                        "{id}: {what} names a home of its own, but it is a node type's item"
                    )));
                }
                (None, None) => {
                    return Err(Error::refused(format!(
                        "{id}: {what} names no home, and no node type is called so"
                    )));
                }
                (Some(home), None) => home,
                (None, Some(_)) => continue,
            };
            let mut places: Vec<Place<'_>> = self
                .context(package, item.context)
                .map(Place::Network)
                .into_iter()
                .collect();
            if item.objects {
                places.push(Place::Objects);
            }
            fits(&what, home, &places)?;
        }
        if !family {
            return Ok(());
        }
        let first = (self.families.iter()).find(|(label, _)| **label == package.label);
        match first {
            Some((label, other)) => Err(Error::refused(format!(
                "{id} lists in a family labelled {label}, as {other} does"
            ))),
            None => Ok(()),
        }
    }

    /// Whether `package` lists anything in its family (what
    /// [`check_homes`](Self::check_homes) has passed).
    pub(super) fn has_family(&self, package: &Package) -> bool {
        let listings = (package.node_types.iter())
            .filter_map(|t| t.description().listing)
            .map(|l| l.home);
        let creations = package.creations.iter().map(|c| c.home.clone());
        let objects = (self
            .object_creations(package)
            .unwrap_or_default()
            .into_iter())
        .map(|c| c.home);
        let items = (package.ui.iter().flat_map(|ui| ui.items())).filter_map(|i| i.home.clone());
        (listings.chain(creations).chain(objects).chain(items)).any(|h| h.topic.is_none())
    }

    /// Network context `id`: `package`'s own or a registered one.
    fn context<'a>(&'a self, package: &'a Package, id: ContextId) -> Option<&'a NetworkContext> {
        (package.contexts.iter())
            .chain(self.contexts.values().map(|e| &*e.item))
            .find(|c| c.id == id)
    }

    /// Node type `id`'s description: `package`'s own or a registered one's.
    fn description(&self, package: &Package, id: NodeTypeId) -> Option<Description> {
        let own = package.node_types.iter().find(|t| t.id() == id);
        let registered = || self.node_types.get(&id).map(|e| e.item.clone());
        own.cloned().or_else(registered).map(|t| t.description())
    }

    /// The places a node type described as `d` lists at its own home: its
    /// contexts, and every network where its reach goes beyond them (a
    /// context adopting it names its home there itself).
    fn offered<'a>(&'a self, package: &'a Package, d: &Description) -> Vec<Place<'a>> {
        let mut places: Vec<Place<'_>> = (d.contexts.iter())
            .filter_map(|&c| self.context(package, c))
            .map(Place::Network)
            .collect();
        if matches!(d.reach, Reach::Everywhere | Reach::Carried) {
            places.push(Place::Everywhere);
        }
        places
    }

    /// The creations `package` lists at the object levels: its kinds' (made
    /// with every engine registered and its own) and its creations of other
    /// packages' kinds, each of a kind registered or its own.
    pub(super) fn object_creations(&self, package: &Package) -> Result<Vec<KindCreation>, Error> {
        let engines: Vec<EngineDescription> = (self.engine_descriptions.iter())
            .chain(&package.engines)
            .map(|e| EngineDescription::clone(e))
            .collect();
        let engines = Engines::new(engines);
        let cx = CreateCx::new(&engines);
        let mut out: Vec<KindCreation> = (package.object_kinds.iter())
            .flat_map(|k| k.creations(&cx))
            .collect();
        for (kind, creation) in &package.object_creations {
            let known = package.object_kinds.iter().any(|k| k.id() == *kind)
                || self.object_kinds.contains_key(kind);
            if !known {
                return Err(Error::refused(format!(
                    "{}'s object creation {} is of kind {kind}, which no package declares",
                    package.id, creation.id
                )));
            }
            out.push(creation.clone());
        }
        Ok(out)
    }
}

/// Whether `home`, which `package` names for `what`, fits each of `places`.
fn fits(package: PackageId, what: &str, home: &Home, places: &[Place<'_>]) -> Result<(), Error> {
    let refused = |why: String| Err(Error::refused(format!("{package}: {what} {why}")));
    if home.groups.iter().any(|g| g.trim().is_empty()) {
        return refused("names a blank group".into());
    }
    match home.topic {
        None if package.as_str() == CORE => {
            refused("names a family: core lists under topics".into())
        }
        None => Ok(()),
        Some(topic) => match places.iter().find(|p| !p.has(topic)) {
            Some(place) => refused(format!("names topic {topic}, which {} lack", place.name())),
            None => Ok(()),
        },
    }
}
