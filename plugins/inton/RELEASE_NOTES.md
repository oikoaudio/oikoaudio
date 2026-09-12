# Oiko Inton 0.5.0-beta.1

Wow, Weft and Inton now share one release version. These plugins are still maturing; sound, controls and automation mappings may change between beta releases. Keep the previous plugin version and a backup of existing projects before updating.

## Changes

- Corner-drag resizing from 50% to 200%, with zoom restored when reopening the editor.
- The updated shared editor and plugin framework, including host state handling and macOS window sizing.
- Release archives include the MTS runtime, installation instructions and third-party notices.

## Compatibility

This release does not change Inton's parameter identities or Scale set slot numbering. Check tuning recall and automated scale changes in an existing project before saving over it. MTS-ESP tuning still requires the runtime and a compatible instrument.

## Formats and known limitations

CLAP and VST3 for Linux x86-64, Windows x86-64, and universal macOS. Builds are unsigned and macOS builds are not notarized. Audio Unit remains disabled while the Logic editor issue is unresolved.

[Manual and installation](https://oikoaudio.com/inton/)
