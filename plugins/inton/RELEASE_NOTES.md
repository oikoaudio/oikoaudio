# Oiko Inton 0.5.0-beta.1

Wow, Weft and Inton now share one release version. Sound, controls and automation mappings may change between beta releases. Before updating, keep a copy of the previous plugin version and back up existing projects.

## Changes

- You can resize the editor from 50% to 200% by dragging its corner. The editor reopens at the last zoom level.
- Inton uses the updated shared editor and plugin framework, which changes host state handling and macOS window sizing.
- Release archives now include the MTS runtime, installation instructions and third-party notices.

## Compatibility

This release does not change Inton's parameter identities or Scale set slot numbering. Before saving over an existing project, check that it recalls its tunings and that automated scale changes still play. MTS-ESP tuning still needs the runtime and a compatible instrument.

## Formats and known limitations

Inton ships as CLAP and VST3 for Linux x86-64, Windows x86-64 and universal macOS. Builds are unsigned, and macOS builds are not notarized. Audio Unit stays disabled until the editor problem in Logic is fixed.

[Manual and installation](https://oikoaudio.com/inton/)
