# Oiko Audio

Rust audio plugins and their shared libraries. One Cargo workspace contains the build inputs and upstream patches. Oiko code is MIT licensed; third-party code and fonts retain their original notices.

```text
crates/       Shared DSP, UI, plugin integration, and tuning parser
plugins/      Hosted products
vendor/       One maintained copy of each patched shared dependency
xtask/        Common CLAP and VST3 bundler
```

See [WoW](plugins/wow/README.md), [Weft](plugins/weft/README.md), and [Inton](plugins/inton/README.md) for product behavior. Start with the [engineering principles](docs/engineering-principles.md), [architecture](docs/architecture.md), and [upstream patch register](docs/upstream-patches.md) when changing shared code.

## Build and test

Use Rust 1.92 or later. Inton's tuning parser and Weft's MTS client need a C++ compiler; the tuning parser requires C++20. Linux GUI development packages are listed in [.github/workflows/ci.yml](.github/workflows/ci.yml). Commands below work in fish, bash, and PowerShell.

```sh
cargo test --workspace --locked
cargo xtask bundle -p wow-plugin -p spectral-plugin -p inton --release --locked
```

Bundles go into `target/bundled/`. To build one product, select its package:

```sh
cargo xtask bundle wow-plugin --release --locked
```

For universal macOS bundles, install both Rust targets and use the same package selection:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo xtask bundle-universal -p wow-plugin -p spectral-plugin -p inton --release --locked
```

Run `python3 scripts/check_workspace.py --test` for formatting, strict Clippy, dependency checks, workspace tests, individual plugin tests, and patched dependency tests. Add `--offline` when dependencies are cached. On Windows, use the installed Python command, usually `python`.

## Repository boundaries

Shared crates stay independent of product-specific dependencies. WoW does not acquire an MTS dependency because Inton and Weft live here. Embedded-editor integration remains in `oiko-plugin`.

Plugins retain their own versions and release schedules. Release tags use `wow/v…`, `weft/v…`, or `inton/v…`. See the [release guide](docs/releases.md) for build triggers and platform archives. The website and Java `bitwig-oikontrol` remain separate repositories.
