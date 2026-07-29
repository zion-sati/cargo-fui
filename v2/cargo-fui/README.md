# cargo-fui

`cargo-fui` is the first-party project, development, build, and native packaging
tool for [FUI-RS](https://github.com/zion-sati/fui-rs).

FUI-RS is the retained Rust UI SDK. EffinDOM is the shared native and browser
runtime beneath it. `cargo-fui` joins the two: it scaffolds an application,
resolves the EffinDOM runtime version declared by FUI-RS, verifies the downloaded
runtime, builds the selected targets, and emits deployable output.

Native applications do not contain a browser, Electron, or a WebView.

## Platform prerequisites

Install the native compiler and linker for your operating system before
installing `cargo-fui`; Cargo uses them while compiling Rust tools and
applications.

- macOS requires the Xcode Command Line Tools (`xcode-select --install`).
- Windows requires Visual Studio 2022 Build Tools with the **Desktop development
  with C++** workload and a Windows SDK. Windows ARM64 also requires the ARM64
  C++ build tools.
- Linux requires a C/C++ compiler toolchain plus the Vulkan, Fontconfig, D-Bus,
  X11, and Xext development libraries used by the EffinDOM native host.

Web and universal projects additionally require Node.js 24 and the
`wasm32-unknown-unknown` Rust target. Native-only projects do not require
Node.js.

## Quick start

Install stable Rust with [rustup](https://rustup.rs/) first. Rustup includes
Cargo on macOS, Windows, and Linux.

```bash
rustc --version
cargo --version
cargo install --locked cargo-fui
cargo fui new my-app --target universal
cd my-app
cargo fui dev
```

The FUI-RS browser demo is available at
<https://fui-rs-demo.effindom.dev/>. Galaga-RS is an independent community
application built with FUI-RS: <https://github.com/jatm80/galaga-rs>.

## Choose a project shape

### Native

```bash
cargo fui new my-app --target native
```

Creates a Rust application for macOS, Windows, or Linux. It has no Node.js files
and does not require Node.js or npm. Native builds require the platform C++ and
packaging toolchain.

### Web

```bash
cargo fui new my-app --target web
```

Creates a Rust/WASM application and its minimal browser harness. It requires the
`wasm32-unknown-unknown` Rust target and Node.js 24.

### Universal

```bash
cargo fui new my-app --target universal
```

Creates one target-independent retained UI crate with explicit native and web
adapters. `cargo fui build` builds both outputs. Use this when the same product
should run as a native desktop application and in a browser.

## Commands

```bash
cargo fui dev                   # debug build, then launch or serve
cargo fui build                 # debug output
cargo fui build --release       # optimized output
cargo fui package               # release DMG, MSIX, or AppImage
cargo fui package --debug       # explicit development package
cargo fui build --offline       # require cached dependencies and runtime
```

`cargo fui dev` favours iteration speed. Release optimisation only happens when
requested explicitly.

Run command-specific help with:

```bash
cargo fui help new
cargo fui help dev
cargo fui help build
cargo fui help package
```

## Runtime acquisition

Native builds derive the exact EffinDOM runtime from FUI-RS package metadata,
download it outside the project, and verify its release manifest and checksums.
Packaged applications carry their runtime and do not download one at startup.

The normal online build populates the cache. `--offline` rejects missing inputs
rather than silently selecting a different runtime.

## Requirements

All targets require stable Rust and Cargo. The supported installation path is
[rustup](https://rustup.rs/); operating-system Rust packages may be too old for
the current FUI-RS toolchain.

See [Platform prerequisites](#platform-prerequisites) before installing
`cargo-fui`. AppImage packaging additionally requires AppImage and SquashFS
tooling.

The generated README contains target-specific setup and run instructions.

## Application metadata

Application identity, caption, source icon, assets, and platform package
metadata live in `fui.toml`. Cargo remains authoritative for the Rust package
name and version.

The source icon is a square PNG at least 256 by 256 pixels. Packaging generates
the platform-specific icon resources.

## Output

Build output is staged under `target/fui/`. `cargo fui package` emits the native
platform format:

- macOS: DMG containing the application bundle.
- Windows: MSIX.
- Linux: AppImage.

Package records and checksums accompany release packages.

## Current status and limitations

This is an early release. Breaking changes remain possible before 1.0.

- Native support currently targets desktop macOS, Windows, and Linux, not iOS or
  Android.
- Production signing identities, notarization, store accounts, and distribution
  policy are supplied and controlled by the application owner.
- Universal projects share UI code, not arbitrary platform services. Native and
  web capabilities remain explicit adapters.
- The supported CI matrix cannot cover every Linux desktop environment, GPU, and
  driver combination.
- The first online build needs network access to resolve crates and the verified
  runtime. Reproducible offline builds require a populated cache.

Report setup failures with the operating system, architecture, tool versions,
the complete command, and the first relevant error.

## Related projects

- FUI-RS SDK: <https://github.com/zion-sati/fui-rs>
- EffinDOM runtime: <https://github.com/zion-sati/EffinDOM>
- Browser demo: <https://fui-rs-demo.effindom.dev/>
- Galaga-RS: <https://github.com/jatm80/galaga-rs>
