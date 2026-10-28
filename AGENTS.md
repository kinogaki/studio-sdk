# Repository working conventions

This public repository is Kinogaki Studio's plugin SDK: the public API Studio and every package (studio-mesh, studio-cad, third parties) compile against. Treat every public item as public API.

- Every public item has rustdoc that says what it is for and the rules an implementation follows: who calls it, from which thread, what it may assume, what it must return. `missing_docs` is on and docs build with `-D warnings`.
- The SDK is self-contained: it never depends on a private Kinogaki crate (Studio, a kernel, or any lib-*), and Studio adapts at the boundary. Every type the API exposes is defined here (`math`, `params`, `element`, `viewport`, `render`, `parallel`). It stays headless: no UI and no GPU API. Studio's `tools/check_architecture.py` enforces this.
- Host UI interfaces go in a separate `studio-ui-api` crate, not here.
- Owned Rust code forbids unsafe.
- No backward compatibility before the public release: no deprecated aliases, no shims, and no version fields in anything this API stores. Change the API and update its users in the same landing (Studio, studio-mesh, studio-cad).
- It builds from a plain clone: no sibling checkouts, no path dependencies outside this repository. It has no third-party crates; one that is truly needed is pinned to an exact version and frozen into this repository's own `vendor/`, never resolved from crates.io.
- Nothing private goes in: no local paths, internal hostnames, credentials, or references to private repositories' contents.
- Keep Cargo.lock. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` before committing. Push here before updating Studio's pin.
- Verification runs locally only. Do not add GitHub Actions workflows or CI credentials unless the user explicitly asks for them.
- Spelling: Color, never the British spelling, in code, docs and commits.
