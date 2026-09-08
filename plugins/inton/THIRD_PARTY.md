# Dependency and factory provenance

Oiko Inton source is MIT licensed (see `LICENSE`).

| Component | Upstream / revision | License and use |
| --- | --- | --- |
| Surge Synth Team tuning-library | https://github.com/surge-synthesizer/tuning-library — `48422e2f014fcda8dd5d1a4678bb2674faf3bb3e` | MIT; C++20 headers behind `oiko-tuning/native/tuning.cpp`. The source notice is `../../vendor/tuning-library/LICENSE.md`; releases include it as `licenses/tuning-library/LICENSE.md`. |
| ODDsound MTS-ESP | https://github.com/ODDSound/MTS-ESP — `f214739b8832e7f297cb9970d0c0efbf783f1462` | ISC-style permission to use/copy/modify/distribute, with or without fee; original `vendor/mts-esp/LICENSE` included. Official Linux shared libraries are supplied unchanged, separately from the plugin. |
| clap-sys 0.5.0 | https://github.com/micahrj/clap-sys | MIT/Apache-2.0; CLAP ABI types. |
| egui 0.36.1 | https://github.com/emilk/egui | MIT/Apache-2.0; user interface and bundled default fonts (see upstream notices). |
| egui-baseview 0.7.0 | https://codeberg.org/RustAudio/egui-baseview | MIT/Apache-2.0; X11/OpenGL editor backend. |
| baseview 0.3.2 | https://github.com/RustAudio/baseview | MIT/Apache-2.0; embedded Linux editor window. |
| Serde / serde_json | https://github.com/serde-rs | MIT/Apache-2.0; state and library metadata. |
| libloading | https://github.com/nagisa/rust_libloading | ISC; load the official MTS runtime. |

`Cargo.lock` pins the complete Rust dependency graph. Each release generates `dependency-licenses.json` from the locked normal/build dependency graph and includes the corresponding notices in `licenses/`, including NicePlug and the shared tuning-library license. The inventory covers conditional targets, so some entries may not be linked into a particular platform build. `docs/dependency-licenses.json` is the checked-in inventory snapshot; release packaging regenerates it rather than copying that snapshot. These notices apply independently of the Oiko Inton MIT license.

The default font bundle also includes Ubuntu Font Licence and SIL Open Font License notices. The original font notices are under `licenses/epaint_default_fonts-0.36.1/fonts/`.

## Factory tuning data

All 49 factory SCL files were independently constructed for Oiko Inton using `scripts/generate-factory.py`. The metadata and generated mathematical data are dedicated under CC0-1.0: https://creativecommons.org/publicdomain/zero/1.0/ . Each resource carries its provenance, and metadata retains source/attribution fields.

Equal divisions use `1200 × log2(period) × step/count`. Just intonation resources state their ratios directly. The Pythagorean resource uses explicit ratios of powers of 2 and 3. Quarter-comma meantone uses a chain of fifths of size `1200 × log2(5^(1/4))`, reduced modulo an octave.

## MTS ABI and thread safety

The dynamic symbols and calling conventions used in `crates/inton-core/src/mts.rs` are those used by the official `Master/libMTSMaster.cpp` wrapper at the pinned revision. The runtime's public documentation specifies `/usr/local/lib` on Linux and permits redistribution of the library. It does not guarantee realtime-safe writes or mandate an update cadence. Inton therefore confines all MTS calls to a control thread and uses the bulk table API.

## Interface font

All Oiko editors use the kit's Ubuntu Regular font in both themes. It is distributed under the Ubuntu Font Licence 1.0; see `licenses/Ubuntu-Font-Licence-1.0.txt`. Oiko Weft's MIT-licensed palette and typography setup are reused for consistency.

The ten Electronic / Detuned 12-note resources are independently specified in `scripts/electronic-detunings.json`. Each SCL degree is 100 times its chromatic index plus the corresponding original cent offset; the octave is exactly 1200 cents. The data is CC0-1.0.


Carlos Alpha/Beta/Gamma are independently generated from the rounded step sizes published at https://www.wendycarlos.com/resources/pitch.html (78.0, 63.8, 35.1 cents). No article text or third-party SCL is copied. The resources use 9, 11, and 20 steps respectively as non-octave periods.

Local tuning-library patch: the KBM lexer permits a leading minus sign on the reference-note field. Rust validates that field as an integer within −256…255 before calling C++; the wrapper retains bounded mappings and complete 128-note validation. Existing upstream notices remain intact.

NicePlug handles the CLAP state envelope and requests `CLAP_PARAM_RESCAN_VALUES` after loading state so the host refreshes cached parameter values even when the editor is closed. The shared local state-validation preflight lets Inton reject malformed project data before applying it. See the root patch register for the maintained wrapper changes.

Patched Rust/native sources and font data live in the root workspace. See `../../docs/upstream-patches.md` for exact sources and upstream candidates.
