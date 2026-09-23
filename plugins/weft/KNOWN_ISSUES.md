# Known issues

## Replacing development builds while a host is running

Overwriting a plugin binary does not reset existing instances. A host may also keep the previous binary or cached plugin metadata loaded in its plugin process. If a newly copied development build opens with unexpected settings, remove the old instance completely and insert a new one. If the host still shows duplicate or stale entries, restart it or rescan its plugins.

Weft's graph-level `RESET` button clears only the drawn spectral curve. It does not restore every parameter to its factory default.

## Initial CLAP editor opening in REAPER

In REAPER on Linux, the Weft CLAP editor may stay hidden the first time it opens. Toggle REAPER's **UI** control off and back on to show it.

The cause could be REAPER, NICE-PLUG's CLAP wrapper, the editor backend, or the Linux windowing path, and it is not yet known which. An upstream report waits on a comparison with the VST3 build and a minimal NICE-PLUG editor.
