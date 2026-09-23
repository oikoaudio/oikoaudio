# Dependency and factory provenance

Oiko Inton source is licensed under either the MIT License or the Apache License 2.0, at your option. See `LICENSE-MIT` and `LICENSE-APACHE`.

| Component | Upstream and revision | License and use |
| --- | --- | --- |
| Surge Synth Team tuning-library | https://github.com/surge-synthesizer/tuning-library at `48422e2f014fcda8dd5d1a4678bb2674faf3bb3e` | MIT. C++20 headers called through `oiko-tuning/native/tuning.cpp`. The source notice is `../../vendor/tuning-library/LICENSE.md`. Releases include it as `licenses/tuning-library/LICENSE.md`. |
| ODDSound MTS-ESP | https://github.com/ODDSound/MTS-ESP at `f214739b8832e7f297cb9970d0c0efbf783f1462` | ISC-style permission to use, copy, modify and distribute, with or without fee. The original `vendor/mts-esp/LICENSE` is included. The official Linux shared libraries are supplied unchanged and separately from the plugin. |
| clap-sys 0.5.0 | https://github.com/micahrj/clap-sys | MIT/Apache-2.0. CLAP ABI types. |
| egui 0.36.1 | https://github.com/emilk/egui | MIT/Apache-2.0. User interface and bundled default fonts (see upstream notices). |
| egui-baseview 0.7.0 | https://codeberg.org/RustAudio/egui-baseview | MIT/Apache-2.0. X11/OpenGL editor backend. |
| baseview 0.3.2 | https://github.com/RustAudio/baseview | MIT/Apache-2.0. Embedded Linux editor window. |
| Serde / serde_json | https://github.com/serde-rs | MIT/Apache-2.0. State and library metadata. |
| libloading | https://github.com/nagisa/rust_libloading | ISC. Loads the official MTS runtime. |

`Cargo.lock` pins the complete Rust dependency graph. Each release generates `dependency-licenses.json` from the locked normal and build dependency graph. The release includes the matching notices in `licenses/`, among them NicePlug and the shared tuning-library license. The inventory covers conditional targets, so a particular platform build may not link every listed entry. `docs/dependency-licenses.json` is the checked-in snapshot of the inventory. Release packaging regenerates the inventory and does not copy that snapshot. These notices apply independently of the Oiko Inton MIT license.

The default font bundle also includes Ubuntu Font Licence and SIL Open Font License notices. The original font notices are under `licenses/epaint_default_fonts-0.36.1/fonts/`.

## Factory tuning data

`scripts/generate-factory.py` constructed all 49 factory SCL files independently for Oiko Inton. The metadata and generated mathematical data are dedicated under CC0-1.0: https://creativecommons.org/publicdomain/zero/1.0/ . Each resource records its provenance, and the metadata keeps its source and attribution fields.

Equal divisions use `1200 × log2(period) × step/count`. Just intonation resources state their ratios directly. The Pythagorean resource uses explicit ratios of powers of 2 and 3. Quarter-comma meantone uses a chain of fifths of size `1200 × log2(5^(1/4))`, reduced modulo an octave.

`scripts/electronic-detunings.json` specifies the ten Electronic › Detuned 12-note resources independently. Each SCL degree is 100 times its chromatic index plus the matching original cent offset. The octave is exactly 1200 cents. The data is CC0-1.0.

Carlos Alpha, Beta and Gamma are generated independently from the rounded step sizes published at https://www.wendycarlos.com/resources/pitch.html (78.0, 63.8 and 35.1 cents). No article text or third-party SCL is copied. The resources use 9, 11 and 20 steps respectively as non-octave periods.

## MTS ABI and thread safety

`crates/inton-core/src/mts.rs` uses the same dynamic symbols and calling conventions as the official `Master/libMTSMaster.cpp` wrapper at the pinned revision. The runtime's public documentation specifies `/usr/local/lib` on Linux and permits redistribution of the library. It does not guarantee realtime-safe writes or require an update rate. Inton therefore makes every MTS call from a control thread and uses the bulk table API.

## Interface font

All Oiko editors use the Ubuntu Regular font from the shared `oiko-ui` crate in both themes. It is distributed under the Ubuntu Font Licence 1.0. See `licenses/Ubuntu-Font-Licence-1.0.txt`. Inton uses the same palette and typography setup as Oiko Weft.

## Local patches

The local tuning-library patch lets the KBM lexer accept a leading minus sign in the reference-note field. Rust checks that the field is an integer from −256 to 255 before calling C++. The wrapper keeps bounded mappings and checks all 128 notes. Existing upstream notices remain intact.

NicePlug handles the CLAP state envelope. After loading state, it requests `CLAP_PARAM_RESCAN_VALUES` so the host refreshes cached parameter values even when the editor is closed. The shared local state-validation check lets Inton reject malformed project data before applying it. The root patch register lists the maintained wrapper changes.

The root workspace holds the patched Rust and native sources and the font data. See `../../docs/upstream-patches.md` for exact sources and upstream candidates.
