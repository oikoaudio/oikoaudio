# Oiko Audio

Free, open-source audio plugins for macOS, Windows and Linux, in CLAP and VST3 formats.

**[Download from oikoaudio.com](https://oikoaudio.com/downloads/)** · [Report a bug](https://github.com/oikoaudio/oikoaudio/issues/new?template=bug-report.yml)

| Plugin | What it does | Manual |
| --- | --- | --- |
| **Weft** | A spectral filter. Free-draw filter shapes, animate them, and play resonances with partials from incoming notes. MTS-ESP and MPE compatible. | [Weft](plugins/weft/README.md) |
| **Wow** | Wow and flutter by modulating playback speed. Windowed-sinc resampling keeps aliasing low while the pitch moves. | [Wow](plugins/wow/README.md) |
| **Inton** | Sends tunings to MTS-ESP instruments. Keeps several scales per project and morphs between them. | [Inton](plugins/inton/README.md) |

> **Public beta.** Sound, controls and automation mappings may change between beta releases. Read each plugin's release notes before updating existing projects. Builds are not signed, and macOS builds are not notarized.

Oikontrol, the Bitwig Studio controller extension, is in the separate [bitwig-oikontrol](https://github.com/oikoaudio/bitwig-oikontrol) repository.

## Development

This repository is one Rust Cargo workspace. It contains the plugins, their shared libraries and the patched upstream dependencies.

```text
crates/       Shared DSP, UI, plugin integration, and tuning parser
plugins/      Hosted products
vendor/       One maintained copy of each patched shared dependency
xtask/        Common CLAP and VST3 bundler
```

The [Wow](plugins/wow/README.md), [Weft](plugins/weft/README.md) and [Inton](plugins/inton/README.md) manuals describe what each plugin does. Before you change shared code, read the [engineering principles](docs/engineering-principles.md), the [architecture](docs/architecture.md) and the [upstream patch register](docs/upstream-patches.md).

### Build and test

Use Rust 1.92 or later. Inton's tuning parser and Weft's MTS client need a C++ compiler, and the tuning parser needs C++20 support. [.github/workflows/ci.yml](.github/workflows/ci.yml) lists the GUI development packages that Linux builds need. The commands below work in fish, bash and PowerShell.

```sh
cargo test --workspace --locked
cargo xtask bundle -p wow-plugin -p spectral-plugin -p inton --release --locked
```

`cargo xtask` writes the bundles to `target/bundled/`. To build one product, select its package:

```sh
cargo xtask bundle wow-plugin --release --locked
```

To build universal macOS bundles, install both Rust targets and select the packages the same way:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo xtask bundle-universal -p wow-plugin -p spectral-plugin -p inton --release --locked
```

`python3 scripts/check_workspace.py --test` checks formatting, runs strict Clippy and the dependency checks, and runs the workspace tests, each plugin's tests on their own and the patched dependency tests. Add `--offline` when the dependencies are already cached. On Windows, use the installed Python command, usually `python`.

### Repository boundaries

The shared DSP, UI and host crates do not depend on product integrations such as MTS-ESP, and `scripts/check_workspace.py` fails if they do. Inton and Weft use MTS-ESP, but Wow does not depend on it. `oiko-plugin` contains the shared NicePlug editor integration, including editor geometry, saved zoom, host gestures and parameter dragging.

Wow, Weft and Inton share one release version, set in `[workspace.package]` in the root `Cargo.toml`. Update the version and release the three plugins together. Pushing one `v<version>` tag builds all three plugins on each platform. The workflow then publishes separate `wow/v…`, `weft/v…` and `inton/v…` releases. The [release guide](docs/releases.md) lists the build triggers and platform archives. The website and the Java `bitwig-oikontrol` project are separate repositories.

## License

Oiko code is licensed under either the [MIT License](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE), at your option. Third-party code and fonts keep their original notices.
