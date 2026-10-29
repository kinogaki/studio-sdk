//! A plugin library's entry point: the one function Studio looks up in it.
//!
//! A plugin is a Rust dylib built with the toolchain and SDK build of the
//! Studio release it targets: the host and every plugin share one standard
//! library and one copy of each shared crate, so the entry hands its
//! [`Package`] over as a plain Rust value. Studio reads the plugin's
//! manifest and checks that it was built for this Studio before it loads
//! the library at all; it then calls the entry once, and never unloads the
//! library.
//!
//! [`export_package!`](crate::export_package) writes the entry, so a
//! plugin crate never writes `unsafe` itself and keeps
//! `unsafe_code = "forbid"`.
use crate::Package;

/// The name of the entry symbol ([`Entry`]).
pub const SYMBOL: &str = "kinogaki_package";

/// The entry's type: it makes the package, or reports the panic that
/// stopped it (Studio then lists the plugin as unavailable).
pub type Entry = fn() -> std::thread::Result<Package>;

/// Exports `make` (a `fn() -> Package`) as the plugin's entry point
/// ([`SYMBOL`](crate::entry::SYMBOL), of type [`Entry`]), catching a panic
/// in it.
///
/// ```ignore
/// fn package() -> kinogaki_studio_api::Package {
///     kinogaki_studio_api::Package::new(
///         kinogaki_studio_api::PackageId::declared("acme.trees"),
///         "Trees",
///     )
/// }
/// kinogaki_studio_api::export_package!(package);
/// ```
#[macro_export]
macro_rules! export_package {
    ($make:path) => {
        /// The entry point Studio looks up in this plugin's library.
        #[unsafe(no_mangle)]
        pub fn kinogaki_package() -> ::std::thread::Result<$crate::Package> {
            let make: fn() -> $crate::Package = $make;
            ::std::panic::catch_unwind(make)
        }
        // The entry has the type the host calls it as.
        const _: $crate::entry::Entry = kinogaki_package;
    };
}
