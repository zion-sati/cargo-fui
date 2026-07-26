# cargo-fui

[![CI](https://github.com/zion-sati/cargo-fui/actions/workflows/ci.yml/badge.svg)](https://github.com/zion-sati/cargo-fui/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/cargo-fui)](https://crates.io/crates/cargo-fui)
[![docs.rs](https://img.shields.io/docsrs/cargo-fui)](https://docs.rs/cargo-fui)

First-party project, development, build, and native packaging tooling for
[FUI-RS](https://github.com/zion-sati/fui-rs).

```bash
cargo install cargo-fui
cargo fui new my-app --target universal
cd my-app
cargo fui dev
```

Targets:

- `native`: Rust plus the platform C++ toolchain; Node.js is not required.
- `web`: Rust/WASM plus Node.js for the browser harness.
- `universal`: one retained Rust UI targeting native and web.

`cargo fui build --release` creates optimized output. `cargo fui package`
uses the same verified EffinDOM runtime and emits the platform-native package
format: DMG, MSIX, or AppImage.

Native-only projects do not contain or require Node.js. Universal projects use
one target-independent retained UI crate and explicit native/web adapters, so
platform services do not leak into the shared UI.

See the [cargo-fui crate guide](v2/cargo-fui/README.md) for commands,
requirements, offline behavior, package metadata, and output details.

## Contributors

See [CONTRIBUTING.md](CONTRIBUTING.md) for repository layout and health gates.
