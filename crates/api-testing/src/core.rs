//! A stand-in for core's package, holding what the SDK's contract names of
//! it: core's network contexts with their topics ([`topics`]) and its
//! Render Output kind ([`render::RENDER_OUTPUT`]). A package's tests register into
//! [`registry`] to be admitted as Studio admits them: their homes in core's
//! places checked against core's topics, an engine's Render Output made of
//! core's kind.
use kinogaki_studio_api::element::{ElementReader, ElementWriter};
use kinogaki_studio_api::menu::topics;
use kinogaki_studio_api::network::NetworkContext;
use kinogaki_studio_api::render::{self, SHADING};
use kinogaki_studio_api::{
    ContextId, DataTypeId, Error, ObjectBody, ObjectCodec, ObjectKindId, ObjectKindSpec, Package,
    PackageId, ReadCx, Registry,
};
use std::sync::Arc;

/// Core's Comp context, inside a Render Output.
const COMP: ContextId = ContextId::declared("kinogaki.core.comp");
/// Core's Materials context, inside a Material Library.
pub const MATERIALS: ContextId = ContextId::declared("kinogaki.core.materials");

/// A registry holding the stand-in core package ([`package`]), as Studio's
/// holds core's before any plugin.
pub fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(package())
        .expect("the stand-in core is admitted");
    registry
}

/// The stand-in core package (`kinogaki.core`): Comp, Shading and
/// Materials with core's topics, and a Render Output kind making nothing.
pub fn package() -> Package {
    Package::new(PackageId::declared("kinogaki.core"), "Studio")
        .with_context(NetworkContext::new(COMP, "Comp", "comp").topics(topics::comp::all()))
        .with_context(NetworkContext::new(SHADING, "Material", "shading"))
        .with_context(
            NetworkContext::new(MATERIALS, "Materials", "materials")
                .topics(topics::materials::all())
                .library(),
        )
        .with_object_kind(RenderOutput)
}

/// A Render Output stand-in: unplaced, at the root, holding Comp.
struct RenderOutput;

#[derive(Debug)]
struct Body;

impl ObjectBody for Body {
    fn kind(&self) -> ObjectKindId {
        render::RENDER_OUTPUT
    }
}

impl ObjectKindSpec for RenderOutput {
    fn id(&self) -> ObjectKindId {
        render::RENDER_OUTPUT
    }
    fn label(&self) -> &str {
        "Render Output"
    }
    fn icon(&self) -> &str {
        "render"
    }
    fn placed(&self) -> bool {
        false
    }
    fn context(&self) -> Option<ContextId> {
        Some(COMP)
    }
    fn result(&self) -> Option<DataTypeId> {
        None
    }
    fn new_body(&self) -> Arc<dyn ObjectBody> {
        Arc::new(Body)
    }
    fn codec(&self) -> &dyn ObjectCodec {
        self
    }
    fn at_root(&self) -> bool {
        true
    }
}

/// Saves nothing.
impl ObjectCodec for RenderOutput {
    fn write(&self, _: &dyn ObjectBody, _: &mut dyn ElementWriter) -> Result<(), Error> {
        Ok(())
    }
    fn read(&self, _: &dyn ElementReader, _: &ReadCx<'_>) -> Result<Arc<dyn ObjectBody>, Error> {
        Ok(Arc::new(Body))
    }
}
