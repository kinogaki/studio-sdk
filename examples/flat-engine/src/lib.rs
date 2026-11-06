//! Flat: an example render engine. It ray-casts the triangles a host sends,
//! through the view's camera, on the CPU, and shades each hit by a light at
//! the eye (its facing ratio) times its material's base color, an OpenPBR
//! Surface's constant or image (the MaterialX nodes it implements; a host
//! adopts every other material into them); what it renders shows through a
//! texture it writes on the host's device. It declares no lights, no nodes of
//! its own and one output, the beauty, and renders each view once a change.
//!
//! Its parts are those of every engine: a description ([`description`]),
//! what it paints materials as (`paint`), a render thread over the SDK's
//! plumbing (`render`), and a display (`display`). `export_package!` hands
//! the package to Studio.
mod display;
mod paint;
mod render;

use kinogaki_render_api::display::{Engine, Made, Options, wgpu};
use kinogaki_render_api::geometry::Error;
use kinogaki_render_api::{
    AovSpec, Capabilities, EngineDescription, EngineId, PrimKind, Wake, aov,
};
use kinogaki_studio_api::{Package, PackageId, export_package};

/// The engine's id, under the example's vendor.
pub const ENGINE: EngineId = EngineId::declared("example.flat.engine");

/// Flat, described.
pub fn description() -> EngineDescription {
    EngineDescription::builder(ENGINE, "Flat")
        .capabilities(Capabilities {
            prims: vec![
                PrimKind::Triangles,
                PrimKind::Instancer,
                PrimKind::Camera,
                PrimKind::Material,
                PrimKind::Texture,
            ],
            standard_nodes: paint::NODES.map(std::sync::Arc::from).to_vec(),
            aovs: vec![AovSpec::new(aov::standard::beauty(), true)],
            // Every pass is the whole frame.
            realtime: true,
            // The film's pixels, as floats and halves, and its texture.
            film_bytes: 32,
            ..Capabilities::default()
        })
        .build()
}

/// Flat as a host makes it.
pub struct Flat;

impl Engine for Flat {
    fn description(&self) -> EngineDescription {
        description()
    }

    fn make(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        wake: Wake,
        _options: &Options,
    ) -> Result<Made, Error> {
        let engine = render::start(wake).map_err(|e| Error::invalid(e.to_string()))?;
        let display = display::Textures::new(device.clone(), queue.clone());
        Ok((Box::new(engine), Box::new(display)))
    }
}

/// The package carrying Flat.
pub fn package() -> Package {
    Package::new(PackageId::declared("example.flat"), "Flat").with_engine(Flat)
}

// The entry point Studio looks up in the plugin's library.
export_package!(package);

#[cfg(test)]
mod tests;
