# Oiko Weft 0.4.0-beta.1

## Changes since 0.1.1-beta4

- Moves Weft into the shared Oiko Audio workspace, with shared DSP and UI infrastructure and framework fixes.
- Aligns the beta version with Wow and Inton.

## Previous beta highlights

- Follows tuning from MTS-ESP hosts such as Inton, including changes to held and pinned notes.
- Shows the active scale on the note ruler, with notes spaced by their tuned frequencies.
- Adds three-bar and dotted-whole sync rates for spectral motion.

Pitch bend and note expression work on top of the MTS tuning. Without an active MTS master, Weft uses standard tuning. Install the MTS-ESP runtime supplied with your tuning software, then restart your DAW.

## Formats and known issues

CLAP and VST3 for Linux x86-64, Windows x86-64, and macOS Apple Silicon and Intel. The beta builds are unsigned and the macOS builds are not notarized. Audio Unit remains disabled while the Logic editor issue is unresolved.

In Bitwig Studio 6.1, CLAP latency may not refresh after changing Resolution. Deactivate and reactivate Weft after a change, especially before exporting. VST3 updates latency correctly in the reported tests. On Linux, REAPER's CLAP editor may need its UI toggled off and back on the first time it opens.

Weft's undo history is separate from the DAW's project history. The Linux host-shortcut forwarding limitation is not fixed by local undo.

[Manual and downloads](https://oikoaudio.com/weft/)
