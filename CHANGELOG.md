# Changelog

## 0.4.0 - 2026-11-03

- cargo fui build and cargo fui dev run the project's generate:host npm script, when package.json defines one, before compiling any WebAssembly, so host-service and host-event bindings follow their contracts.
- cargo fui build, dev and package map the builder's home directory to ~ in the source paths rustc records, such as panic locations, so published WebAssembly no longer names it. Native executables can still contain paths that C code compiled by a dependency's build script records.
- Routed web apps built with cargo fui show the loading overlay on every route; their pages previously shipped the overlay's unfilled template placeholders.
- Release web builds are optimized for size: web and universal projects build Rust at opt-level "z", and every route and Worker module then goes through Binaryen's wasm-opt -Oz, which strips its function names and toolchain sections.

## 0.1.12 - 2026-10-30

- Updated verified upstream dependency pins.

## 0.1.11 - 2026-08-09

- Added first-class routed web and universal scaffolds to cargo-fui.

## 0.1.10 - 2026-08-08

- Updated verified upstream dependency pins.

## 0.1.7 - 2026-08-07

- Updated downstream SDKs, tooling, scaffolders, and demos for EffinDOM 0.2.12.

## 0.1.9 - 2026-08-07

- Updated verified upstream dependency pins.

## 0.1.8 - 2026-08-07

- Fixed hard refreshes on clean routed URLs served by cargo-fui.
