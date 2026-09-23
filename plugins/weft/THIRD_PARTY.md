# Third-party notices

Oiko Weft source is licensed under either the MIT License or the Apache License 2.0, at your option. See `LICENSE-MIT` and `LICENSE-APACHE`. Releases include `dependency-licenses.json` and `licenses/`, which cover the resolved Rust dependencies, fonts, and the MTS-ESP client. The inventory lists normal and build dependencies for all supported and conditional targets, so a given platform build may not link every entry.

The MTS-ESP client wrapper has one local change to its loader. On Unix, the loader uses an explicit `WEFT_MTS_LIBRARY` path instead of the default paths. If that override fails, the loader never falls back to the system library. Tests use this to avoid a live master.
