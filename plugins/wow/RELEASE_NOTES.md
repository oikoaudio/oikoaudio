# Oiko Wow 0.5.0-beta.1

Wow, Weft and Inton now share one release version. These plugins are still maturing; sound, controls and automation mappings may change between beta releases. Keep the previous plugin version and a backup of existing projects before updating.

## Changes

- Independent host-tempo sync for Wow and Flutter, selected with Hz or the note symbol beside each rate. Switching modes keeps the rate near its previous speed, within each oscillator's range.
- A circular Phase dial with smooth automation and transport anchoring for synced oscillators. Stereo spread appears as an orange arc; drag its degree value or Alt-drag the dial to adjust it. Shift gives fine adjustment.
- A Wow / Flutter balance slider that shows the Wow share, so 90/10 is 90% full.
- Corner-drag resizing from 50% to 200%, restored zoom handling, and the updated shared editor and plugin framework.

## Compatibility with earlier betas

Existing parameter identities and mappings are retained. The balance slider changes presentation, while existing Wow / Flutter automation keeps its meaning. The former L/R Phase Offset remains the stereo spread parameter. New sync controls default to free Hz and the new common phase offset defaults to zero when loading older states.

For projects made with development builds that already had tempo sync, playback starts, loops, seeks and division changes now anchor synced oscillators to song position. Drift still adds variation around that reference. Check those projects if their sound depended on a free-running synced phase.

## Formats and known limitations

CLAP and VST3 for Linux x86-64, Windows x86-64, and universal macOS. Builds are unsigned and macOS builds are not notarized. Audio Unit remains disabled while the Logic editor issue is unresolved.

[Manual and installation](https://oikoaudio.com/wow/)
