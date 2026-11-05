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
//! channels as they are. Files carry colors in spaces of their own. A picture
//! is the one place past this rule: a Layout page composes display-referred
//! pieces, so its Text's color and a display-referred background are the target
//! display's code values, and a placed file keeps what it holds. The host
//! holds the color maths (its configuration's conversions and view
//! transforms, a file's space, display profiles) and lends the service to
//! formats (`Reading`, `Writing`), cooks (the project they read) and
//! runners; a package never carries a copy of it.
//!
//! Spaces are named as the host's configuration names them, or by its
//! aliases (`lin_rec709`, `srgb_texture`, `ACEScg`).

use crate::{Diagnostic, Error};
use std::fmt::Debug;

/// The host's color management.
pub trait ColorService: Send + Sync + Debug {
    /// The project's working space, by its name in the configuration.
    fn working(&self) -> &str;

    /// Converts `rgb` in place from space `from` to space `to`.
    ///
    /// # Errors
    /// A space the configuration doesn't name, or no conversion between
    /// them; `rgb` is then unchanged.
    fn convert(&self, from: &str, to: &str, rgb: &mut [[f64; 3]]) -> Result<(), Error>;

    /// Converts `rgb` in place from space `from` to the working space.
    ///
    /// # Errors
    /// As [`convert`](Self::convert).
    fn to_working(&self, from: &str, rgb: &mut [[f64; 3]]) -> Result<(), Error> {
        self.convert(from, self.working(), rgb)
    }

    /// Converts `rgb` in place from the working space to space `to`.
    ///
    /// # Errors
    /// As [`convert`](Self::convert).
    fn working_to(&self, to: &str, rgb: &mut [[f64; 3]]) -> Result<(), Error> {
        self.convert(self.working(), to, rgb)
    }

    /// Whether colors in `space` are display-referred: a display's code values,
    /// which reach a picture as they are rather than through a view. No, by
    /// default.
    fn display_referred(&self, _space: &str) -> bool {
        false
    }

    /// Changes whenever a conversion would.
    fn revision(&self) -> u64;

    /// The spaces a working space can be (the config's scene-linear
    /// ones), by name; by default the working space alone.
    fn spaces(&self) -> Vec<String> {
        vec![self.working().to_owned()]
    }

    /// The displays a picture can be made for: standard dynamic range, with a
    /// tag a written file carries; by name, none by default.
    fn target_displays(&self) -> Vec<String> {
        Vec::new()
    }

    /// What its configuration got wrong (an unreadable config, a space it
    /// lacks), with what stands in; none by default.
    fn problem(&self) -> Option<Diagnostic> {
        None
    }
}
