# Repository working conventions

This public repository is Kinogaki Studio's plugin SDK: the public API Studio and every plugin (Studio's own and third parties') compile against. Treat every public item as public API.

- Every public item has rustdoc that says what it is for and the rules an implementation follows: who calls it, from which thread, what it may assume, what it must return. `missing_docs` is on and docs build with `-D warnings`.
- The SDK is self-contained: it never depends on a private Kinogaki crate (Studio, a kernel, or any lib-*), and Studio adapts at the boundary. Every type the API exposes is defined here (`math`, `params`, `element`, `viewport`, `render`, `parallel`, and the leaf crates `render-api` and `viewport-api`). `crates/api` stays headless: no UI, and no GPU API but its opt-in `wgpu` feature, which carries packages' render engines (render-api's display contract) and nothing else. Studio's architecture check enforces this.
- UI goes in `crates/ui-api` (`kinogaki-studio-ui-api`), never in `crates/api`. It defines its own drawing and widget types; plugins never see Studio's UI library or egui types, and the host draws.
- Owned Rust code forbids unsafe.
- No backward compatibility before the public release: no deprecated aliases, no shims, and no version fields in anything this API stores. Change the API and update its users in the same landing (Studio and its plugins, and this repository's examples and walkthroughs).
- It builds from a plain clone: no sibling checkouts, no path dependencies outside this repository. Its third-party dependencies are wgpu, behind `kinogaki-render-api`'s `wgpu` feature, and bytemuck, behind `kinogaki-viewport-api`'s `bytemuck` feature: normal crates.io dependencies pinned to exact versions (`Cargo.lock` is committed). Anything else third-party is a deliberate decision, never a convenience; nothing is copied into this repository.
- Nothing private goes in: no local paths, internal hostnames, credentials, or names of or references to private repositories and libraries. Name Studio's parts generically ("Studio's Mesh plugin", "its mesh kernel").
- The docs are part of the API: a change to what a plugin writes updates `docs/ARCHITECTURE.md` and the walkthroughs (`docs/FIRST-NODE-PLUGIN.md`, `docs/FIRST-CONTEXT-PLUGIN.md`) in the same commit, and the examples they quote.
- Keep Cargo.lock. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` before committing. Push here before updating Studio's pin.
- Verification runs locally only. Do not add GitHub Actions workflows or CI credentials unless the user explicitly asks for them.
- Spelling: Color, never the British spelling, in code, docs and commits.
