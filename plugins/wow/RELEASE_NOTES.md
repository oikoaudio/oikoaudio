# Oiko Wow 0.5.0-beta.1

Wow, Weft and Inton now share one release version. These plugins are still in beta, so sound, controls and automation mappings may change between beta releases. Before updating, keep the previous plugin version and back up existing projects.

## Changes

- Wow and Flutter each sync to host tempo independently. Select Hz or the note symbol beside each rate. Switching modes keeps the rate near its previous speed, within that oscillator's range.
- A new circular Phase dial rotates both oscillators. Its automation is smoothed, and synced oscillators anchor to the transport. Stereo spread appears as an orange arc around the dial. Drag its degree value or Alt-drag the dial to adjust it, and hold Shift for fine adjustment.
- The Wow / Flutter balance is now a slider that shows the Wow share, so 90/10 is 90% full.
- Drag the window corner to resize from 50% to 200%. The editor reopens at the chosen size. Wow also uses the updated shared editor and plugin framework.

## Compatibility with earlier betas

Parameter identities and mappings are unchanged. The balance slider changes only the display, so existing Wow / Flutter automation keeps its meaning. The L/R Phase Offset parameter is still the stereo spread. When Wow loads an older state, the new sync controls default to free Hz and the new common phase offset defaults to zero.

Some development builds already had tempo sync. In projects made with them, synced oscillators now anchor to song position when playback starts, loops or seeks, and when the division changes. Drift still adds variation around that position. Check those projects if their sound depended on a free-running synced phase.

## Formats and known limitations

This release includes CLAP and VST3 for Linux x86-64, Windows x86-64, and universal macOS. Builds are unsigned, and macOS builds are not notarized. There is no Audio Unit build until the Logic editor crash is fixed.

[Manual and installation](https://oikoaudio.com/wow/)
