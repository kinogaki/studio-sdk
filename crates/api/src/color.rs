//! Color management, the host's: one [`ColorService`] lent to formats
//! ([`Reading`](crate::Reading), [`Writing`](crate::Writing)), cooks
//! ([`CookCx::colors`](crate::CookCx::colors)), the UI and compiled
//! contexts' runners.
//!
//!
//! **One rule.** Authored colors (parameters, attributes, lights' and
//! settings' values, material constants) are linear in the project's
//! working space, wherever they are held: objects, render setups, a
//! `Value::Color` in a body. Colors
//! reaching an engine (prims, materials, render settings) are linear
//! ACEScg (`kinogaki_render_api::RENDER_SPACE`), and engines output it: the host converts
//! each at that one boundary, as it sends it, through this service, and
//! sends them again when the working space or the configuration changes.
//! The same types carry both, so an adapter between authored kinds copies
//! channels as they are. Files carry colors in spaces of their own. The host
//! holds the color maths (its configuration's conversions and view
//! transforms) and lends the service to formats (`Reading`, `Writing`),
//! cooks (the project they read) and runners; a package never carries a
//! copy of it.
//!
//! Spaces are named as the host's configuration names them, or by its
//! aliases (`lin_rec709`, `srgb_texture`, `ACEScg`).

use std::fmt::Debug;
use std::sync::Arc;

/// The host's color management.
pub trait ColorService: Send + Sync + Debug {
    /// The project's working space, by its name in the configuration.
    fn working(&self) -> &str;

    /// Converts `rgb` in place from space `from` to space `to`.
    ///
    /// # Errors
    /// A space the configuration doesn't name, or no conversion between
    /// them; `rgb` is then unchanged.
    fn convert(&self, from: &str, to: &str, rgb: &mut [[f64; 3]]) -> Result<(), String>;

    /// Converts `rgb` in place from space `from` to the working space.
    ///
    /// # Errors
    /// As [`convert`](Self::convert).
    fn to_working(&self, from: &str, rgb: &mut [[f64; 3]]) -> Result<(), String> {
        self.convert(from, self.working(), rgb)
    }

    /// Converts `rgb` in place from the working space to space `to`.
    ///
    /// # Errors
    /// As [`convert`](Self::convert).
    fn working_to(&self, to: &str, rgb: &mut [[f64; 3]]) -> Result<(), String> {
        self.convert(self.working(), to, rgb)
    }

    /// `view`'s transform as WGSL named `name` (`[A-Za-z0-9_]`) reading its
    /// data at `group`, `binding`, for a runner that shows display code
    /// values (Comp's To Display); none when the configuration lacks it.
    fn view_transform(
        &self,
        view: &ViewChoice,
        name: &str,
        group: u32,
        binding: u32,
    ) -> Option<ViewTransform>;

    /// The space the configuration's file rules give the file at `path`
    /// (an image's colors are in it); none when no rule names one, by
    /// default.
    fn file_space(&self, _path: &str) -> Option<String> {
        None
    }

    /// Changes whenever a conversion, a file's space or a view transform
    /// would.
    fn revision(&self) -> u64;

    /// The spaces a working space can be (the config's scene-linear
    /// ones), by name; by default the working space alone.
    fn spaces(&self) -> Vec<String> {
        vec![self.working().to_owned()]
    }

    /// What its configuration got wrong (an unreadable config, a space it
    /// lacks), with what stands in; none by default.
    fn problem(&self) -> Option<String> {
        None
    }
}

/// Which display and view a view transform is for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ViewChoice {
    /// The viewer's display and view.
    Viewer,
    /// A display and view of the host's color configuration, by name.
    Pinned {
        /// The display.
        display: Arc<str>,
        /// The view.
        view: Arc<str>,
    },
}

/// A view transform (scene-referred ACEScg to display code values, without
/// the viewer's exposure and gamma) as the host's color management writes
/// it for a compute shader: WGSL declaring its data buffer at the group and
/// binding asked for, with helpers and a function `fn <function>(c:
/// vec3<f32>) -> vec3<f32>`, and the buffer's contents. The host holds the
/// color maths; a package only embeds the shader.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewTransform {
    /// The WGSL: the buffer's declaration, helpers and the function.
    pub source: String,
    /// The function's name.
    pub function: String,
    /// The buffer's contents (f32s).
    pub data: Vec<f32>,
}
