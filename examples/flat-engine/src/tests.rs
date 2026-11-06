//! Flat renders what a host sends: the SDK's engine conformance (a GPU
//! test).
use super::Flat;
use kinogaki_render_api::display::{Engine, Options};
use kinogaki_studio_api_testing::engine::{conformance, device};

#[test]
#[ignore = "requires a native graphics adapter"]
fn flat_renders_what_a_host_sends() {
    let (device, queue) = device();
    let make = |wake| Flat.make(&device, &queue, wake, &Options::default());
    let report = conformance("Flat", &make);
    eprintln!("{report}");
    assert!(report.failures().is_empty(), "{report}");
}
