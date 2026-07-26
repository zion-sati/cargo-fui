# cargo-fui

[![CI](https://github.com/zion-sati/cargo-fui/actions/workflows/ci.yml/badge.svg)](https://github.com/zion-sati/cargo-fui/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/cargo-fui)](https://crates.io/crates/cargo-fui)
[![docs.rs](https://img.shields.io/docsrs/cargo-fui)](https://docs.rs/cargo-fui)

`cargo-fui` creates, runs, builds, and packages
[FUI-RS](https://github.com/zion-sati/fui-rs) applications for native desktop,
the browser, or both from one retained Rust UI codebase.

It does not wrap a website in Electron or a WebView. Native projects compile
against the EffinDOM native runtime; web projects compile the application to
WebAssembly and use the EffinDOM browser runtime.

## Start here

Install stable Rust with [rustup](https://rustup.rs/). Rustup includes Cargo on
macOS, Windows, and Linux. Confirm both commands are available:

```bash
rustc --version
cargo --version
```

Then install the Cargo subcommand:

```bash
cargo install --locked cargo-fui
```

Create one UI that runs natively and in the browser:

```bash
cargo fui new my-app --target universal
cd my-app
cargo fui dev
```

Use `--target native` when you only need desktop, or `--target web` when you
only need a browser application.

- [Run the FUI-RS browser demo](https://fui-rs-demo.effindom.dev/)
- [Read the complete cargo-fui guide](v2/cargo-fui/README.md)
- [Read the FUI-RS SDK documentation](https://github.com/zion-sati/fui-rs)
- [Play Galaga-RS](https://jatm80.github.io/galaga-rs/), the first known
  community-built FUI-RS application

## What each project is

| Project | Role |
| --- | --- |
| **FUI-RS** | The application-facing retained Rust UI SDK: controls, layout, themes, text, input, drawing, events, accessibility semantics, and platform APIs. |
| **EffinDOM** | The shared native and WebAssembly runtime that performs layout, text shaping, rendering, input routing, and semantic projection. |
| **cargo-fui** | The project generator and build/package tool that acquires the matching verified runtime and produces native or web output. |

Generated applications depend on FUI-RS. FUI-RS metadata pins the compatible
EffinDOM runtime, and `cargo-fui` verifies that runtime before using it. You do
not manually match runtime versions.

## Choose a target

| Target | Use it when | Tooling |
| --- | --- | --- |
| `native` | The application only needs macOS, Windows, or Linux desktop output. | Stable Rust and the platform C++/packaging toolchain. Node.js is not required. |
| `web` | The application only needs browser/WASM output. | Stable Rust, `wasm32-unknown-unknown`, and Node.js 24. |
| `universal` | Shared retained UI should produce both native and browser applications. | The native and web requirements above. |

A universal project contains a target-independent UI crate plus small native
and web adapters. Platform services remain explicit rather than leaking into
shared UI code.

## Main commands

```bash
cargo fui dev                   # debug build, then launch or serve
cargo fui build                 # debug output
cargo fui build --release       # optimized output
cargo fui package               # release DMG, MSIX, or AppImage
cargo fui package --debug       # explicit development package
cargo fui build --offline       # use only cached dependencies and runtime
```

Run `cargo fui help` or `cargo fui help <command>` for command-specific help.

## Current status

FUI-RS and `cargo-fui` are early releases. They are usable enough to build and
package real applications, but pre-1.0 APIs and generated project structure may
still change when correctness or developer experience requires it.

Current boundaries:

- Native targets are desktop macOS, Windows, and Linux. iOS and Android are not
  currently supported.
- Web and universal projects require Node.js for the browser harness; native-only
  projects do not contain or require Node.js.
- The first online build downloads the FUI-RS crate and its pinned EffinDOM
  runtime. `--offline` works after those inputs have been cached.
- Production signing identities, store accounts, notarization, and distribution
  policy remain application-owner responsibilities.
- CI covers the supported operating-system and architecture matrix, but cannot
  represent every Linux distribution, desktop environment, GPU, or driver.

If one of these boundaries blocks a real application, open a discussion before
building a large workaround around it.

## Contributors

See [CONTRIBUTING.md](CONTRIBUTING.md) for repository layout, health gates, and
small first-contribution paths.
