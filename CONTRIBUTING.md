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

## Good first contributions

Useful first contributions do not require changing the build orchestrator:

- Reproduce the published quickstart on a platform or Linux distribution not
  already represented in an open issue and document any missing prerequisite.
- Improve an error message with the failing command, expected recovery, and a
  regression test.
- Correct generated-project documentation where it differs from actual output.
- Add a focused packaging acceptance case for an already supported metadata
  field.

Open an issue describing the intended change before starting broader platform
or packaging work.
