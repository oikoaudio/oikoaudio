# Oiko Audio shared foundation

A Cargo workspace containing the shared DSP, UI, host integration and tuning libraries, the CLAP/VST3 bundler, and their pinned dependencies. Oiko source is MIT licensed; dependency and font notices remain alongside the corresponding sources.

Run from the repository root:

```sh
cargo build --workspace --locked --offline
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
```

Omit `--offline` when initially fetching the locked dependencies. The workspace requires Rust 1.92 or later, a C++20 compiler for the tuning parser and platform GUI development libraries for the editor integration.

The maintained dependencies and patch artifacts are recorded in [the patch register](docs/upstream-patches.md). Local investigations are kept outside tracked source under ignored `.scratch/`.
