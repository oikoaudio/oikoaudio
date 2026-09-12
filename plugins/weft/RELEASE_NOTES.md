# Oiko Weft 0.5.0-beta.1

Wow, Weft and Inton now share one release version. These plugins are still maturing; sound, controls and automation mappings may change between beta releases. Keep the previous plugin version and a backup of existing projects before updating.

## Compatibility with earlier betas

Check Motion Shape automation before playing or exporting existing projects. Cloud expands the selector from seven choices to eight. VST3 stores automation as normalized positions, so some old automation points now select a different shape. Host mappings and macros that store normalized values may also need adjustment.

| Previous Motion Shape | Shape selected by the old VST3 automation value |
| --- | --- |
| Ripple | Ripple |
| Harmonic | Harmonic |
| Drift | Drift |
| Scan | Notch |
| Notch | Saw |
| Saw | Sprinkle |
| Splash | Cloud |

Re-select the intended shape and update the affected automation points or controller mappings. Review transitions between points as well. Normalized positions changed from index/6 to index/7; a host's value labels are the easiest way to check the result.

Saved integer shape selections and CLAP's plain parameter values retain their indices. Index 6, formerly Splash, now selects Sprinkle. Sprinkle replaces Splash with a different particle effect, so those saved settings will sound different even without automation. Keep the previous version if you need the original Splash sound. Loading plugin state cannot migrate automation stored by the host.

## Changes

- Sprinkle replaces Splash with deterministic spectral particles. Cloud adds rounded and reverse-swell particle envelopes. Both work with live or pinned notes and generate free patterns when no notes are supplied.
- Common per-note expression and MPE support for tuning, pressure, gain, pan, brightness and vibrato.
- Corner-drag resizing from 50% to 200%, restored zoom handling, and the updated shared editor and plugin framework.

MTS-ESP tuning support continues to follow changes to held and pinned notes. Pitch bend and note expression work on top of that tuning. Without an active MTS master, Weft uses standard tuning.

## Formats and known limitations

CLAP and VST3 for Linux x86-64, Windows x86-64, and universal macOS. Builds are unsigned and macOS builds are not notarized. Audio Unit remains disabled while the Logic editor issue is unresolved.

In reported Bitwig Studio 6.1 tests, CLAP latency may not refresh after changing Resolution. Deactivate and reactivate Weft after a change, especially before exporting. VST3 updated latency correctly in those tests. On Linux, REAPER's CLAP editor may need its UI toggled off and back on the first time it opens.

Weft's undo history is separate from the DAW's project history. The Linux host-shortcut forwarding limitation is not fixed by local undo.

[Manual and installation](https://oikoaudio.com/weft/)
