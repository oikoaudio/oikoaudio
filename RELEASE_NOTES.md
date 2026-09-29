# Oiko 0.5.0-beta.2

This release covers Wow, Weft and Inton. Sound, controls and automation mappings may change between beta releases. Before updating, keep the previous plugin versions and back up existing projects.

## Changes

- On Linux, the editors now open at the right size on desktops that scale X11 apps, such as KDE Plasma on Wayland at 200% with legacy apps scaling themselves. Before, the host window kept the unscaled size and cropped the enlarged editor. With VST3, the window can open at the smaller size and then grow to fit the editor.

## Compatibility

Parameter identities, mappings, Inton's Scale set slot numbering and saved state are unchanged from 0.5.0-beta.1. If you are updating Weft from an earlier beta, check the Motion Shape automation notes in the [Weft 0.5.0-beta.1 release](https://github.com/oikoaudio/oikoaudio/releases/tag/weft/v0.5.0-beta.1).

## Formats and known limitations

Each plugin ships as CLAP and VST3 for Linux x86-64, Windows x86-64 and universal macOS. Inton's archives also contain the MTS runtime. Builds are unsigned, and macOS builds are not notarized. There is no Audio Unit build until the Logic editor crash is fixed.

On Linux, Weft's CLAP editor in REAPER may stay hidden the first time it opens. Toggle REAPER's UI control off and back on to show it.

Weft's undo history is separate from the DAW's project history. Local undo does not fix the Linux limitation on forwarding host shortcuts.

Manuals and installation: [Wow](https://oikoaudio.com/wow/), [Weft](https://oikoaudio.com/weft/), [Inton](https://oikoaudio.com/inton/).
