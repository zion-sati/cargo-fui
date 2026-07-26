# cargo-fui

First-party project, build, development, and native packaging tooling for
[FUI-RS](https://github.com/zion-sati/fui-rs).

## Quick start

```bash
cargo install cargo-fui
cargo fui new my-app --target universal
cd my-app
cargo fui dev
```

Choose a project shape explicitly:

- `native` creates a Rust static-library application with no Node.js files.
- `web` creates a Rust/WASM application and the minimal browser harness.
- `universal` creates one target-independent UI crate with separate native and
  web adapters and service boundaries.

## Commands

```bash
cargo fui dev                   # debug build, then launch or serve
cargo fui build                 # debug output
cargo fui build --release       # optimized output
cargo fui package               # release DMG, MSIX, or AppImage
cargo fui package --debug       # explicit development package
cargo fui build --offline       # require cached dependencies and runtime
```

Native builds derive the exact EffinDOM runtime from FUI-RS package metadata,
download it outside the project, verify its release manifest and checksums, and
package it with the application. Packaged applications do not download a
runtime at startup.

## Requirements

All targets require stable Rust. Native targets also require the platform C++
and packaging toolchain: Xcode command-line tools on macOS, Visual Studio and
the Windows SDK on Windows, or a C++ compiler plus AppImage/SquashFS tooling on
Linux. Web and universal development additionally require Node.js 24 or later.

Application identity, caption, source icon, assets, and platform package
metadata live in `fui.toml`. Cargo remains authoritative for the package name
and version.
