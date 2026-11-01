//! Color management, the host's: one [`ColorService`] for every package.
//! Scene colors (parameters, attributes, lights, materials) are linear in
//! the project's working space; engines take and output linear ACEScg
//! ([`RENDER_SPACE`]); files carry colors in spaces of their own. The host
//! holds the color maths (its configuration's conversions and view
//! transforms) and lends the service to formats (`Reading`, `Writing`),
//! cooks (the project they read) and runners; a package never carries a
//! copy of it.
//!
//! Spaces are named as the host's configuration names them, or by its
//! aliases (`lin_rec709`, `srgb_texture`, `ACEScg`).

use std::fmt::Debug;
use std::sync::Arc;

/// The space engines take and output: linear ACEScg.
pub const RENDER_SPACE: &str = "ACEScg";

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

    /// Changes whenever a conversion or a view transform would.
    fn revision(&self) -> u64;
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
