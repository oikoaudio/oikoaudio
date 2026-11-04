# Oiko 0.5.1

This release covers Wow, Weft and Inton, which are still in beta. Releases now use plain version numbers, so this one follows 0.5.0-beta.3. Sound, controls and automation mappings may change between beta releases. Before updating, keep the previous plugin versions and back up existing projects.

## Changes

- On Windows with display scaling above 100%, the VST3 editors no longer crash hosts that resize the plugin window while the editor opens. In 0.5.0-beta.2 and beta.3, opening the editor at 150% could close the host.
- On Windows with display scaling above 100%, the CLAP editors now report their real size, so the host window fits the editor instead of cropping it. This happened in every earlier beta.
- Wow's resampling now costs less CPU. Its output differs from 0.5.0-beta.3 by less than -150 dB.

## Compatibility

Parameter identities, mappings, Inton's Scale set slot numbering and saved state are unchanged from 0.5.0-beta.3. If you are updating Inton from 0.5.0-beta.2 or earlier, read the folder move notes in the [Inton 0.5.0-beta.3 release](https://github.com/oikoaudio/oikoaudio/releases/tag/inton/v0.5.0-beta.3). If you are updating Weft from an earlier beta, check the Motion Shape automation notes in the [Weft 0.5.0-beta.1 release](https://github.com/oikoaudio/oikoaudio/releases/tag/weft/v0.5.0-beta.1).

## Formats and known limitations

Each plugin ships as CLAP and VST3 for Linux x86-64, Windows x86-64 and universal macOS. Inton's archives also contain the MTS runtime. Builds are unsigned, and macOS builds are not notarized. There is no Audio Unit build until the Logic editor crash is fixed.

On Linux, Weft's CLAP editor in REAPER may stay hidden the first time it opens. Toggle REAPER's UI control off and back on to show it.

Weft's undo history is separate from the DAW's project history. Local undo does not fix the Linux limitation on forwarding host shortcuts.

Manuals and installation: [Wow](https://oikoaudio.com/wow/), [Weft](https://oikoaudio.com/weft/), [Inton](https://oikoaudio.com/inton/).
