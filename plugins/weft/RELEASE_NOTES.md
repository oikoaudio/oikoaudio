# Oiko Weft 0.5.0-beta.1

Wow, Weft and Inton now share one release version. These plugins are still in beta, so sound, controls and automation mappings may change between beta releases. Before updating, keep the previous plugin version and back up existing projects.

## Compatibility with earlier betas

Check Motion Shape automation before playing or exporting existing projects. Cloud expands the selector from seven choices to eight. VST3 stores automation as normalized positions, so some old automation points now select a different shape. Host mappings and macros that store normalized values may also need updating.

| Previous Motion Shape | Shape selected by the old VST3 automation value |
| --- | --- |
| Ripple | Ripple |
| Harmonic | Harmonic |
| Drift | Drift |
| Scan | Notch |
| Notch | Saw |
| Saw | Sprinkle |
| Splash | Cloud |

Re-select the intended shape and update the affected automation points or controller mappings. Also check the transitions between points. Normalized positions changed from index/6 to index/7. The host's value labels show which shape each point now selects.

Saved integer shape selections and CLAP's plain parameter values keep their indices. Index 6, formerly Splash, now selects Sprinkle. Sprinkle is a different particle effect from Splash, so those saved settings sound different even without automation. Keep the previous version if you need the original Splash sound. Loading plugin state cannot migrate automation, because the host stores it.

## Changes

- Sprinkle replaces Splash and generates deterministic spectral particles.
- The new Cloud shape adds rounded and reverse-swell particle envelopes.
- Sprinkle and Cloud follow live or pinned notes. Without notes, they generate their own patterns.
- Weft now responds to common per-note expression and MPE for tuning, pressure, gain, pan, brightness and vibrato.
- Drag the window corner to resize from 50% to 200%. The editor reopens at the chosen size. Weft also uses the updated shared editor and plugin framework.

MTS-ESP tuning still follows changes to held and pinned notes. Pitch bend and note expression add to that tuning. Without an active MTS master, Weft uses standard tuning.

## Formats and known limitations

This release includes CLAP and VST3 for Linux x86-64, Windows x86-64, and universal macOS. Builds are unsigned, and macOS builds are not notarized. There is no Audio Unit build until the Logic editor crash is fixed.

On Linux, the CLAP editor in REAPER may stay hidden the first time it opens. Toggle REAPER's UI control off and back on to show it.

Weft's undo history is separate from the DAW's project history. Local undo does not fix the Linux limitation on forwarding host shortcuts.

[Manual and installation](https://oikoaudio.com/weft/)
