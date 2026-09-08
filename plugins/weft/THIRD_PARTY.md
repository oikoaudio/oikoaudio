# Third-party notices

Oiko Weft source is MIT licensed; see `LICENSE`. Releases include `dependency-licenses.json` and `licenses/` for the resolved Rust dependencies, fonts, and the MTS-ESP client. The inventory includes normal and build dependencies across supported and conditional targets; some entries may not be linked into a particular platform build.

The MTS-ESP client wrapper has one local loader change: on Unix, an explicit `WEFT_MTS_LIBRARY` path is used instead of the default paths. Failed overrides never fall back to the system library. This allows tests to avoid a live master.
