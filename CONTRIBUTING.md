# Contributing

Install stable Rust and the native toolchain for your operating system.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Native acceptance additionally requires the platform packaging tools: Xcode
command-line tools on macOS, the Windows SDK on Windows, or AppImage tooling,
SquashFS tools, and Xvfb on Linux.
