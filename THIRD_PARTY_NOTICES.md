# Third-party notices

## egui

Parts of `kinogaki-studio-ui-api` (`crates/ui-api`) follow [egui](https://github.com/emilk/egui) 0.36.2 formula for formula, so that a host built on egui converts their values bit for bit:

- `src/geom.rs`: the arithmetic of `Pos`, `Vec2`, `Rect` and `Align2`, after egui's `emath` (`Pos2`, `Vec2`, `Rect`, `Align2`);
- `src/color.rs`: `Color`'s constructors, gamma and linear multiplication, blending, opacity and the sRGB conversions, after egui's `ecolor` (`Color32`, `Rgba`), with its tests.

egui is Copyright (c) 2018-2021 Emil Ernerfeldt <emil.ernerfeldt@gmail.com> and the egui contributors, dual-licensed under the MIT license ([licenses/egui-LICENSE-MIT](licenses/egui-LICENSE-MIT)) or the Apache License 2.0 ([licenses/egui-LICENSE-APACHE](licenses/egui-LICENSE-APACHE)), at your option.

## Dependencies

The SDK contains no third-party code beyond the above. Its default build depends on the Rust standard library alone. Two optional features bring crates.io dependencies, which carry their own licenses and are not copied into this repository:

- [wgpu](https://crates.io/crates/wgpu) 30.0.1 (MIT OR Apache-2.0), with `kinogaki-render-api`'s `wgpu` feature (and `kinogaki-studio-api`'s, which turns it on);
- [bytemuck](https://crates.io/crates/bytemuck) 1.25.2 (Zlib OR Apache-2.0 OR MIT), with `kinogaki-viewport-api`'s `bytemuck` feature.

A build with those features also uses the crates they depend on. The table lists them for every target platform, as `Cargo.lock` pins them (a build links only its own platform's); "Linked" says whether a crate's code ends up in a build, or only runs while building (procedural macros and what they use). It is what this command prints:

```sh
cargo tree --locked --workspace -e normal --target all --prefix none --format "{p}|{l}" \
    --features kinogaki-render-api/wgpu,kinogaki-viewport-api/bytemuck,kinogaki-studio-api/wgpu
```

| Crate | Version | License | Linked |
|---|---|---|---|
| [allocator-api2](https://crates.io/crates/allocator-api2) | 0.2.21 | MIT OR Apache-2.0 | yes |
| [android_system_properties](https://crates.io/crates/android_system_properties) | 0.1.6 | MIT OR Apache-2.0 | yes |
| [arrayvec](https://crates.io/crates/arrayvec) | 0.7.8 | MIT OR Apache-2.0 | yes |
| [ash](https://crates.io/crates/ash) | 0.38.0+1.3.281 | MIT OR Apache-2.0 | yes |
| [bit-set](https://crates.io/crates/bit-set) | 0.10.0 | Apache-2.0 OR MIT | yes |
| [bit-vec](https://crates.io/crates/bit-vec) | 0.9.1 | Apache-2.0 OR MIT | yes |
| [bitflags](https://crates.io/crates/bitflags) | 2.13.2 | MIT OR Apache-2.0 | yes |
| [block2](https://crates.io/crates/block2) | 0.6.2 | MIT | yes |
| [bumpalo](https://crates.io/crates/bumpalo) | 3.20.3 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [bytemuck](https://crates.io/crates/bytemuck) | 1.25.2 | Zlib OR Apache-2.0 OR MIT | yes |
| [bytemuck_derive](https://crates.io/crates/bytemuck_derive) | 1.12.1 | Zlib OR Apache-2.0 OR MIT | no: build time only (procedural macros) |
| [cfg-if](https://crates.io/crates/cfg-if) | 1.0.5 | MIT OR Apache-2.0 | yes |
| [codespan-reporting](https://crates.io/crates/codespan-reporting) | 0.13.1 | Apache-2.0 | yes |
| [crunchy](https://crates.io/crates/crunchy) | 0.2.4 | MIT | yes |
| [dlib](https://crates.io/crates/dlib) | 0.5.3 | MIT | yes |
| [document-features](https://crates.io/crates/document-features) | 0.2.12 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [equivalent](https://crates.io/crates/equivalent) | 1.0.3 | Apache-2.0 OR MIT | yes |
| [foldhash](https://crates.io/crates/foldhash) | 0.2.0 | Zlib | yes |
| [futures-core](https://crates.io/crates/futures-core) | 0.3.34 | MIT OR Apache-2.0 | yes |
| [futures-task](https://crates.io/crates/futures-task) | 0.3.34 | MIT OR Apache-2.0 | yes |
| [futures-util](https://crates.io/crates/futures-util) | 0.3.34 | MIT OR Apache-2.0 | yes |
| [glow](https://crates.io/crates/glow) | 0.17.0 | MIT OR Apache-2.0 OR Zlib | yes |
| [glutin_wgl_sys](https://crates.io/crates/glutin_wgl_sys) | 0.6.1 | Apache-2.0 | yes |
| [gpu-allocator](https://crates.io/crates/gpu-allocator) | 0.28.0 | MIT OR Apache-2.0 | yes |
| [half](https://crates.io/crates/half) | 2.7.1 | MIT OR Apache-2.0 | yes |
| [hashbrown](https://crates.io/crates/hashbrown) | 0.16.1 | MIT OR Apache-2.0 | yes |
| [hashbrown](https://crates.io/crates/hashbrown) | 0.17.1 | MIT OR Apache-2.0 | yes |
| [indexmap](https://crates.io/crates/indexmap) | 2.14.2 | Apache-2.0 OR MIT | yes |
| [jni-sys](https://crates.io/crates/jni-sys) | 0.3.1 | MIT OR Apache-2.0 | yes |
| [jni-sys](https://crates.io/crates/jni-sys) | 0.4.1 | MIT OR Apache-2.0 | yes |
| [jni-sys-macros](https://crates.io/crates/jni-sys-macros) | 0.4.1 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [js-sys](https://crates.io/crates/js-sys) | 0.3.106 | MIT OR Apache-2.0 | yes |
| [khronos-egl](https://crates.io/crates/khronos-egl) | 6.0.0 | MIT OR Apache-2.0 | yes |
| [libc](https://crates.io/crates/libc) | 0.2.191 | MIT OR Apache-2.0 | yes |
| [libloading](https://crates.io/crates/libloading) | 0.8.9 | ISC | yes |
| [libm](https://crates.io/crates/libm) | 0.2.16 | MIT | yes |
| [litrs](https://crates.io/crates/litrs) | 1.0.1 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [lock_api](https://crates.io/crates/lock_api) | 0.4.14 | MIT OR Apache-2.0 | yes |
| [log](https://crates.io/crates/log) | 0.4.34 | MIT OR Apache-2.0 | yes |
| [naga](https://crates.io/crates/naga) | 30.0.1 | MIT OR Apache-2.0 | yes |
| [naga-types](https://crates.io/crates/naga-types) | 30.0.1 | MIT OR Apache-2.0 | yes |
| [ndk-sys](https://crates.io/crates/ndk-sys) | 0.6.0+11769913 | MIT OR Apache-2.0 | yes |
| [num-traits](https://crates.io/crates/num-traits) | 0.2.19 | MIT OR Apache-2.0 | yes |
| [objc2](https://crates.io/crates/objc2) | 0.6.5 | MIT | yes |
| [objc2-core-foundation](https://crates.io/crates/objc2-core-foundation) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | yes |
| [objc2-core-graphics](https://crates.io/crates/objc2-core-graphics) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | yes |
| [objc2-encode](https://crates.io/crates/objc2-encode) | 4.1.0 | MIT | yes |
| [objc2-foundation](https://crates.io/crates/objc2-foundation) | 0.3.2 | MIT | yes |
| [objc2-metal](https://crates.io/crates/objc2-metal) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | yes |
| [objc2-quartz-core](https://crates.io/crates/objc2-quartz-core) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | yes |
| [once_cell](https://crates.io/crates/once_cell) | 1.21.4 | MIT OR Apache-2.0 | yes |
| [ordered-float](https://crates.io/crates/ordered-float) | 5.5.1 | MIT | yes |
| [parking_lot](https://crates.io/crates/parking_lot) | 0.12.5 | MIT OR Apache-2.0 | yes |
| [parking_lot_core](https://crates.io/crates/parking_lot_core) | 0.9.12 | MIT OR Apache-2.0 | yes |
| [pin-project-lite](https://crates.io/crates/pin-project-lite) | 0.2.17 | Apache-2.0 OR MIT | yes |
| [portable-atomic](https://crates.io/crates/portable-atomic) | 1.15.0 | Apache-2.0 OR MIT | yes |
| [portable-atomic-util](https://crates.io/crates/portable-atomic-util) | 0.2.8 | Apache-2.0 OR MIT | yes |
| [presser](https://crates.io/crates/presser) | 0.3.1 | MIT OR Apache-2.0 | yes |
| [proc-macro2](https://crates.io/crates/proc-macro2) | 1.0.107 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [profiling](https://crates.io/crates/profiling) | 1.0.18 | MIT OR Apache-2.0 | yes |
| [quote](https://crates.io/crates/quote) | 1.0.47 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [range-alloc](https://crates.io/crates/range-alloc) | 0.1.5 | MIT OR Apache-2.0 | yes |
| [raw-window-handle](https://crates.io/crates/raw-window-handle) | 0.6.2 | MIT OR Apache-2.0 OR Zlib | yes |
| [raw-window-metal](https://crates.io/crates/raw-window-metal) | 1.1.0 | MIT OR Apache-2.0 | yes |
| [redox_syscall](https://crates.io/crates/redox_syscall) | 0.5.18 | MIT | yes |
| [renderdoc-sys](https://crates.io/crates/renderdoc-sys) | 1.1.0 | MIT OR Apache-2.0 | yes |
| [rustc-hash](https://crates.io/crates/rustc-hash) | 1.1.0 | Apache-2.0 OR MIT | yes |
| [scopeguard](https://crates.io/crates/scopeguard) | 1.2.0 | MIT OR Apache-2.0 | yes |
| [slab](https://crates.io/crates/slab) | 0.4.12 | MIT | yes |
| [slotmap](https://crates.io/crates/slotmap) | 1.1.1 | Zlib | yes |
| [smallvec](https://crates.io/crates/smallvec) | 1.16.4 | MIT OR Apache-2.0 | yes |
| [spirv](https://crates.io/crates/spirv) | 0.4.0+sdk-1.4.341.0 | Apache-2.0 | yes |
| [static_assertions](https://crates.io/crates/static_assertions) | 1.1.0 | MIT OR Apache-2.0 | yes |
| [syn](https://crates.io/crates/syn) | 2.0.119 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [syn](https://crates.io/crates/syn) | 3.0.9 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [thiserror](https://crates.io/crates/thiserror) | 2.0.21 | MIT OR Apache-2.0 | yes |
| [thiserror-impl](https://crates.io/crates/thiserror-impl) | 2.0.21 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [tokio](https://crates.io/crates/tokio) | 1.54.1 | MIT | yes |
| [unicode-ident](https://crates.io/crates/unicode-ident) | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | yes |
| [unicode-width](https://crates.io/crates/unicode-width) | 0.2.3 | MIT OR Apache-2.0 | yes |
| [wasm-bindgen](https://crates.io/crates/wasm-bindgen) | 0.2.129 | MIT OR Apache-2.0 | yes |
| [wasm-bindgen-futures](https://crates.io/crates/wasm-bindgen-futures) | 0.4.79 | MIT OR Apache-2.0 | yes |
| [wasm-bindgen-macro](https://crates.io/crates/wasm-bindgen-macro) | 0.2.129 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [wasm-bindgen-macro-support](https://crates.io/crates/wasm-bindgen-macro-support) | 0.2.129 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [wasm-bindgen-shared](https://crates.io/crates/wasm-bindgen-shared) | 0.2.129 | MIT OR Apache-2.0 | yes |
| [wayland-sys](https://crates.io/crates/wayland-sys) | 0.31.11 | MIT | yes |
| [web-sys](https://crates.io/crates/web-sys) | 0.3.106 | MIT OR Apache-2.0 | yes |
| [wgpu](https://crates.io/crates/wgpu) | 30.0.1 | MIT OR Apache-2.0 | yes |
| [wgpu-core](https://crates.io/crates/wgpu-core) | 30.0.1 | MIT OR Apache-2.0 | yes |
| [wgpu-core-deps-apple](https://crates.io/crates/wgpu-core-deps-apple) | 30.0.1 | MIT OR Apache-2.0 | yes |
| [wgpu-core-deps-emscripten](https://crates.io/crates/wgpu-core-deps-emscripten) | 30.0.1 | MIT OR Apache-2.0 | yes |
| [wgpu-core-deps-windows-linux-android](https://crates.io/crates/wgpu-core-deps-windows-linux-android) | 30.0.1 | MIT OR Apache-2.0 | yes |
| [wgpu-hal](https://crates.io/crates/wgpu-hal) | 30.0.1 | MIT OR Apache-2.0 | yes |
| [wgpu-naga-bridge](https://crates.io/crates/wgpu-naga-bridge) | 30.0.1 | MIT OR Apache-2.0 | yes |
| [wgpu-types](https://crates.io/crates/wgpu-types) | 30.0.1 | MIT OR Apache-2.0 | yes |
| [windows](https://crates.io/crates/windows) | 0.62.2 | MIT OR Apache-2.0 | yes |
| [windows-collections](https://crates.io/crates/windows-collections) | 0.3.2 | MIT OR Apache-2.0 | yes |
| [windows-core](https://crates.io/crates/windows-core) | 0.62.2 | MIT OR Apache-2.0 | yes |
| [windows-future](https://crates.io/crates/windows-future) | 0.3.2 | MIT OR Apache-2.0 | yes |
| [windows-implement](https://crates.io/crates/windows-implement) | 0.60.2 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [windows-interface](https://crates.io/crates/windows-interface) | 0.59.3 | MIT OR Apache-2.0 | no: build time only (procedural macros) |
| [windows-link](https://crates.io/crates/windows-link) | 0.2.1 | MIT OR Apache-2.0 | yes |
| [windows-numerics](https://crates.io/crates/windows-numerics) | 0.3.1 | MIT OR Apache-2.0 | yes |
| [windows-result](https://crates.io/crates/windows-result) | 0.4.1 | MIT OR Apache-2.0 | yes |
| [windows-strings](https://crates.io/crates/windows-strings) | 0.5.1 | MIT OR Apache-2.0 | yes |
| [windows-threading](https://crates.io/crates/windows-threading) | 0.2.1 | MIT OR Apache-2.0 | yes |
| [zerocopy](https://crates.io/crates/zerocopy) | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | yes |
| [zerocopy-derive](https://crates.io/crates/zerocopy-derive) | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | no: build time only (procedural macros) |
