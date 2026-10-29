//! The `wgpu` feature's display contract, as far as it goes without a
//! device: images need a real texture view, so only the plain parts are
//! checked here.
#![cfg(feature = "wgpu")]

use kinogaki_render_api::display::{Display, Engine, FilmImage, Made, Options, Shown, ShownFilm};

#[test]
fn options_default_to_nothing() {
    let options = Options::default();
    assert_eq!(options.cache, None);
    assert!(!options.read_pixels);
}

#[test]
fn engines_and_displays_are_objects_hosts_can_share() {
    fn send<T: Send + ?Sized>() {}
    fn send_sync<T: Send + Sync + ?Sized>() {}
    send::<dyn Display>();
    send::<Made>();
    send::<Shown>();
    send::<ShownFilm>();
    send_sync::<FilmImage>();
    send_sync::<dyn Engine>();
}
