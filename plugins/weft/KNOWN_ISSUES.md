# Known issues

## Resolution changes and latency in Bitwig Studio 6.1 CLAP

Changing Weft's Resolution changes the audible FFT processing immediately, but Bitwig Studio 6.1 may continue to display the latency of the previous resolution. Whether its underlying plug-in delay compensation is also stale is still being measured. Manually deactivate and reactivate the Weft device after changing Resolution, especially before bouncing or exporting. Bitwig then reports the selected latency correctly.

Expected latency by resolution:

| Resolution | Samples | Approx. at 44.1 kHz | Approx. at 48 kHz |
| --- | ---: | ---: | ---: |
| Rough | 1,024 | 23.2 ms | 21.3 ms |
| Coarse | 2,048 | 46.4 ms | 42.7 ms |
| Responsive | 4,096 | 92.9 ms | 85.3 ms |
| Balanced | 8,192 | 185.8 ms | 170.7 ms |
| Precise | 16,384 | 371.5 ms | 341.3 ms |

This currently appears related to the host not completing the CLAP restart requested by the plug-in. The cause has not been confirmed.

The VST3 build updates Bitwig Studio 6.1's reported latency as expected. This makes the observed failure specific to Bitwig's CLAP restart path, although the minimal lifecycle probe is still required before filing the upstream report. REAPER updates the reported latency correctly for both Weft's CLAP and VST3 builds.

## Replacing development builds while a host is running

Overwriting a plug-in binary does not reset existing instances, and a host may keep the previous binary or cached plug-in metadata alive in its plug-in process. If a newly copied development build opens with unexpected settings, remove the old instance completely and insert a new one. Restart or rescan the host if it continues to show duplicate or stale entries.

Weft's graph-level `RESET` button clears only the drawn spectral curve. It does not restore every parameter to its factory default.

## Initial CLAP editor opening in REAPER

In REAPER on Linux, the Weft CLAP editor may not become visible on its first open. Toggle REAPER's **UI** control off and back on; the editor then appears.

This has not yet been isolated to REAPER, NICE-PLUG's CLAP wrapper, the editor backend, or the Linux windowing path. The VST3 build and a minimal NICE-PLUG editor still need comparison before an upstream report is filed.
