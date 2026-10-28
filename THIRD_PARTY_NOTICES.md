# Third-party notices

## egui

Parts of `kinogaki-studio-ui-api` (`crates/ui-api`) follow [egui](https://github.com/emilk/egui) 0.36.2 formula for formula, so that a host built on egui converts their values bit for bit:

- `src/geom.rs`: the arithmetic of `Pos`, `Vec2`, `Rect` and `Align2`, after egui's `emath` (`Pos2`, `Vec2`, `Rect`, `Align2`);
- `src/color.rs`: `Color`'s constructors, gamma and linear multiplication, blending, opacity and the sRGB conversions, after egui's `ecolor` (`Color32`, `Rgba`), with its tests.

egui is Copyright (c) 2018-2021 Emil Ernerfeldt <emil.ernerfeldt@gmail.com> and the egui contributors, dual-licensed under the MIT license ([licenses/egui-LICENSE-MIT](licenses/egui-LICENSE-MIT)) or the Apache License 2.0 ([licenses/egui-LICENSE-APACHE](licenses/egui-LICENSE-APACHE)), at your option.
