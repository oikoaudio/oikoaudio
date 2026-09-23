# Installation and building

Inton builds as CLAP and VST3 plugins on Linux, macOS and Windows. It uses the same `nice-plug-xtask` bundling workflow as Oiko Wow and Oiko Weft.

## Build

All platforms need Rust 1.92 or later and a C++20 compiler. Run source-build commands from the workspace root.

```sh
cargo test --workspace --locked
cargo xtask bundle inton --release
```

The bundler writes `target/bundled/Oiko Inton.clap` and `target/bundled/Oiko Inton.vst3`. On macOS these are ad-hoc-signed bundles with the standard macOS bundle structure. On Linux and Windows the bundler writes the native CLAP and VST3 files for that platform.

Linux also needs X11/OpenGL development libraries. On Debian-family systems, install `libx11-dev libxcursor-dev libx11-xcb-dev libxcb-dri2-0-dev libxcb-icccm4-dev libgl1-mesa-dev libglu1-mesa-dev`. On a Wayland desktop, the editor works when the DAW runs under XWayland.

To build a universal macOS bundle with both Apple Silicon and Intel code, run:

```sh
python3 scripts/release.py build inton --platform macOS
```

The script installs both Rust targets, builds both architectures, combines them with `lipo`, verifies the result, and writes the bundle under `target/bundled`.

On Windows, run the same Cargo command in PowerShell:

```powershell
cargo xtask bundle inton --release
```

## Install or update Inton

Each release ZIP contains the CLAP and VST3 bundles, installation notes, licenses and the official ODDSound MTS runtime. Extract the whole ZIP and follow `README.txt`. The MTS helper leaves an existing runtime installation in place. Windows builds still need manual testing in a DAW.

To create a ZIP after building, run this with Python 3.11 or later:

```sh
python3 scripts/release.py package inton --platform Linux --output dist/inton-linux-x86_64.zip
```

Use `macOS` or `Windows` for the other platforms. GitHub Actions packages each platform automatically. A manual workflow run does not publish a release.

## Install the plugin bundles

Close your DAW before replacing installed bundles. After building, run these commands from the workspace root. For a downloaded release, copy the two bundles from the extracted archive to the same destinations.

On macOS:

```sh
mkdir -p ~/Library/Audio/Plug-Ins/CLAP ~/Library/Audio/Plug-Ins/VST3
cp -Rp "target/bundled/Oiko Inton.clap" ~/Library/Audio/Plug-Ins/CLAP/
cp -Rp "target/bundled/Oiko Inton.vst3" ~/Library/Audio/Plug-Ins/VST3/
```

On Linux:

```sh
mkdir -p ~/.clap ~/.vst3
cp -Rp "target/bundled/Oiko Inton.clap" ~/.clap/
cp -Rp "target/bundled/Oiko Inton.vst3" ~/.vst3/
```

On Windows, use PowerShell:

```powershell
$clapDir = Join-Path $env:LOCALAPPDATA "Programs\Common\CLAP"
$vst3Dir = Join-Path $env:LOCALAPPDATA "Programs\Common\VST3"
New-Item -ItemType Directory -Force $clapDir, $vst3Dir | Out-Null
Copy-Item "target\bundled\Oiko Inton.clap" $clapDir -Recurse -Force
Copy-Item "target\bundled\Oiko Inton.vst3" $vst3Dir -Recurse -Force
```

Copy each bundle as a whole folder. If you use custom plugin folders, copy the bundles there and add those folders to your DAW's scan paths. Restart your DAW, and rescan plugins if Inton does not appear.

## Install the MTS shared library

For a first MTS-ESP installation, close all audio hosts. From the workspace root, run `bash plugins/inton/scripts/install-mts.sh` on Linux or macOS, or `powershell -File plugins/inton/scripts/install-mts.ps1` on Windows. In an extracted release archive, the helpers are under `scripts/`. The helpers use the official ODDSound installers and libraries bundled with Inton, and they leave an existing installation in place.

Inton loads the library from the official location for each platform:

- Linux: `/usr/local/lib/libMTS.so`
- macOS: `/Library/Application Support/MTS-ESP/libMTS.dylib`
- Windows: `%ProgramFiles%\Common Files\MTS-ESP\libMTS.dll`

`INTON_MTS_LIBRARY` overrides the location for development and integration tests. Restart the host after installing the shared library.

## Scale files and preferences

Inton stores its scales and preferences in the standard per-user data directories. These are `~/.local/share` and `~/.config` on Linux, `~/Library/Application Support` on macOS, and `%LOCALAPPDATA%` on Windows. The plugin state embeds every scale a project uses, including its keyboard mapping.

## Developer references

- [Architecture](architecture.md)
- [Testing](../../../docs/testing.md#inton)
- [Third-party licenses](../THIRD_PARTY.md)
