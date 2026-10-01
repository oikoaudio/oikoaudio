# Oiko 0.5.0-beta.3

This release covers Wow, Weft and Inton. Sound, controls and automation mappings may change between beta releases. Before updating, keep the previous plugin versions and back up existing projects.

## Changes

- Wow's Wow / Flutter slider now runs from Wow on the left to Flutter on the right, the same direction as its parameter and host automation. Before, the editor drew it the other way round. The slider is labelled WOW and FLUTTER at its two ends, and the host still shows the exact balance, such as 90/10. Existing projects and automation sound the same.
- New Wow and Weft instances open at the editor zoom you last chose. A saved project still reopens at the zoom it was saved with.
- Inton now keeps its files in an `Oiko Audio/Inton` folder (`oikoaudio/inton` on Linux) in the standard per-user directories. The first time it loads, Inton moves the `oiko/inton` folder that earlier versions used, including your favourites, library and preferences. A project whose current scale came from your library sounds the same, but the browser won't highlight that scale until you choose it again.

## Compatibility

Parameter identities, mappings, Inton's Scale set slot numbering and saved state are unchanged from 0.5.0-beta.2. If you go back to an earlier beta after running this one, Inton won't find the moved folder and opens with its default favourites and library. If you are updating Weft from an earlier beta, check the Motion Shape automation notes in the [Weft 0.5.0-beta.1 release](https://github.com/oikoaudio/oikoaudio/releases/tag/weft/v0.5.0-beta.1).

## Formats and known limitations

Each plugin ships as CLAP and VST3 for Linux x86-64, Windows x86-64 and universal macOS. Inton's archives also contain the MTS runtime. Builds are unsigned, and macOS builds are not notarized. There is no Audio Unit build until the Logic editor crash is fixed.

On Linux, Weft's CLAP editor in REAPER may stay hidden the first time it opens. Toggle REAPER's UI control off and back on to show it.

Weft's undo history is separate from the DAW's project history. Local undo does not fix the Linux limitation on forwarding host shortcuts.

Manuals and installation: [Wow](https://oikoaudio.com/wow/), [Weft](https://oikoaudio.com/weft/), [Inton](https://oikoaudio.com/inton/).
