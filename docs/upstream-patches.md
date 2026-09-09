# Upstream patch register

The remaining patch artifacts below contain local changes; resolved upstream fixes are recorded separately. Edit and test a patch once in this repository. The root workspace patches all consumers to these sources. Retain upstream notices. Do not create product-specific copies.

A local patch is not necessarily an upstream bug fix. Distinguish reproduced failures from product policy, API extensions, and unverified native workarounds. Reproduction against a recorded base does not establish that current upstream main is affected.

| Dependency / base | Shared source | Change and upstream disposition |
| --- | --- | --- |
| NicePlug 0.4.0; published crate, Git commit `ce793d8275aebf7589e35fb5fc1e1e3e6a016940` | `vendor/nice-plug` | Remaining local state-loader alternative checks platform length representability and grows its buffer fallibly in response to chunked reads, without upstream's universal 256 MiB cap. Truncated streams and reported allocation failures return a failed load; valid large envelopes remain supported. Deserialization/decompression allocations and operating-system overcommit remain outside this fix. The state-validation preflight is an API extension, not a confirmed upstream bug. |
| NicePlug core 0.4.0; same Git commit | `vendor/nice-plug-core` | Reproduced parameter-bookkeeping bug: preserve base and normalized values when modulation is clipped or a discrete value does not change, and notify the editor while deduplicating effective-value callbacks. Separately, the default Plugin::validate_state hook is an API extension for product-owned validation. |
| NicePlug egui 0.5.0; same Git commit | crates.io | Published adapter; its Rust source matches the former snapshot exactly. No vendor override. |
| egui-baseview 0.7.0 published crate | `vendor/egui-baseview` | Modifier-only state adjustment is a suspected native bug workaround; backend semantics and real-host reproduction remain unverified. Selective Command/Ctrl shortcut capture is an opt-in API extension. Published 0.7.0 already services resize commands after the UI callback; that ordering is not our fix. The logging delta addresses a reproduced compile failure with both logging backends disabled. Local tracing-only gating does not cover log-only diagnostics; a revised guard has only been tested in an investigation copy. |
| baseview 0.3.2 plus upstream commit `c36ca154f882353f04684973dfe683c1b3d6abb3` | `vendor/baseview` | Existing upstream AppKit tracking-area and cursor fixes, including NSZeroRect with NSTrackingInVisibleRect. Already upstream: wait for a release containing this commit; do not propose the same fix again. |
| Surge tuning-library `48422e2f014fcda8dd5d1a4678bb2674faf3bb3e` | `vendor/tuning-library` | Local compatibility extension accepting a leading minus sign in a KBM reference-note field. Inton's Rust/native boundary validates the supported signed range and the complete result. Not a confirmed specification bug; excluded from upstream reporting. |

## Resolved upstream

Issue 01, first CLAP event applied before its sample offset, is resolved by upstream [PR #91](https://codeberg.org/RustAudio/nice-plug/pulls/91), commit `41e04a1`, included in the 0.4.0 version-bump commit. The local event-loop patch is removed; all eight timing regressions remain and pass against the pristine pinned Git sources as well as the patched vendor sources. State-restore parameter rescanning is also upstream (`f5235690ce`), so its local patch is removed. Upstream's typed `VoiceID`, `Channel`, and `Key` wildcards replace the local `NOTE_ADDRESS_WILDCARD` constant and CLAP output conversion patch.

All products now use `nice_plug_egui::EguiEditor` directly; the redundant Oiko host-coordinate wrapper and handle alias have been removed. Upstream NativeSize owns the host coordinate contract. The separate macOS canvas-zoom workaround remains pending native testing.

## Patch artifacts and tests

- `patches/nice-plug-0.4.0.patch`: CLAP wrapper fixes and common CLAP/VST3 state-validation preflight. Inton's CLAP host tests cover short stream reads/writes, valid recall, parameter rescanning, malformed state, unrepresentable lengths and truncated input without replacing current state. Run `cargo test -p inton --test clap_host --locked` at the workspace root. The vendored `tests/clap_state_stream.rs` exercises large valid envelopes above 256 MiB, chunk boundaries, partial reads, truncation, invalid read counts, and injected allocation failure through a minimal CLAP host. Its allocator checks that a huge length prefix does not trigger a huge reservation; failed loads preserve parameter values. The vendored `tests/clap_event_timing.rs` uses a minimal plugin and CLAP host to check first-event offsets, automation/modulation, transport boundaries, note forwarding, same-sample groups, block-rate opt-out, and buffer-size independence. `scripts/check_workspace.py --test` runs it through a temporary consumer harness with allocation guards enabled. These timing tests historically reproduced against published NicePlug 0.3.0 and now pass with the upstream 0.4.0 event loop, without a local scheduling delta.
- `patches/nice-plug-core-0.4.0.patch`: the matching validation hook and parameter-value fixes. Core regression tests cover upper and lower modulation limits, integer/boolean base edits, quantization, editor notification, and callback deduplication. `scripts/check_workspace.py --test` runs them from a temporary copy. The snapshot includes the ISC notice from the same upstream commit.
- `patches/egui-baseview-0.7.0.patch`: local input behavior and logging guard. Vendor window tests exercise modifier updates and shortcut routing; Weft and Inton editor tests exercise local undo. Native input behavior remains unverified. Resize ordering already exists in the published base; no logging-induced runtime resize crash was demonstrated.
- `patches/tuning-library.patch`: exact header diff against the recorded commit. Run `cargo test -p inton-core --test contract --locked` at the workspace root, including negative-reference validation and tuning fixtures.
- baseview is an unchanged snapshot of the recorded upstream commit. Its upstream Git history is the patch record; host coordinates now use the upstream NicePlug/egui implementation.

## Upgrade validation, 2026-09-09

`python3 scripts/check_workspace.py --test --offline` passed, including warning-denying workspace Clippy, workspace tests, separate product suites, all wrapper harnesses, and core/window vendor regressions. All eight timing cases also passed against pristine NicePlug/core sources at `dc473f2451eecb82b3fd1ab37375ae6ffabc86d3`. Both regenerated NicePlug patch artifacts were applied to pristine snapshots and byte-compared with every vendored Rust source/test. Native DAW checks were not run; macOS NativeSize resizing, lifecycle, and the CLAP/VST3 host matrix still need release validation.

## Published release validation, 2026-09-10

`python3 scripts/check_workspace.py --test --offline` passed against the published crate base: formatting, strict Clippy, workspace and individual product suites, CLAP timing/state/note-expression harnesses, and core/window regressions. The egui adapter resolves from crates.io. Native DAW checks were not run.

## Note expression maintenance

The NicePlug patch also repairs the VST3 Brightness/Expression swap, supplies expression-specific defaults, forwards note-on tuning in semitones, replaces duplicate note-ID cache entries, clears that cache on reset, and forwards ID-only expressions after its bounded channel/key cache wraps. These are maintained local changes, not claims of acceptance or reproduction against current upstream main. Upstream now owns typed wildcard conversion. The remaining CLAP input patch validates the single-port address before narrowing, and the `ClapPlugin::CLAP_SUPPORTS_MPE` flag advertises MPE only for opted-in consumers. Existing plugins retain their default dialects. NicePlug core also exports its STFT input traits so a product can use bounded buffer views without allocation; this is an API extension.

`vendor/nice-plug/tests/note_expressions.rs` covers the actual CLAP entry point and the VST3 expression translator. `scripts/check_workspace.py --test` runs it alongside the existing timing and state-stream harnesses. Weft's product tests cover common expression state, active-note changes, same-sample ordering, wildcard/overlap lifecycle, MPE configuration, stereo spectral contributions and process allocation guards. Native Bitwig routing is not established by these tests.

## Packaging differences

NicePlug/core 0.4.0 now use the published crates.io archives from upstream commit `ce793d8275aebf7589e35fb5fc1e1e3e6a016940`, with the recorded local patches reapplied. Archive SHA-256 checksums were verified against the registry. Published manifests and `Cargo.toml.orig` are retained; packaged Cargo.lock files are omitted because the root lockfile is authoritative. The archives omit LICENSE files, so the previously recorded upstream ISC notices are retained. Both patch artifacts were regenerated against the published Rust sources/tests and reapplied to fresh copies to verify an exact match. NicePlug egui 0.5.0 is loaded directly from crates.io; its Rust sources match the former snapshot, and the temporary vendor override and checksums have been removed.

Relative to the earlier snapshot, the published sources add `ParamSetter::request_restart`, flush pending CLAP output parameter events during processing, and replace several host/state debug assertions with warnings. These upstream changes are retained alongside our local patches.

The egui-baseview vendor snapshot inherited from Weft/Inton is OpenGL-only: its local manifest omits optional wgpu dependencies/examples and renderer.rs selects only OpenGL. These are vendoring choices, not upstream bug fixes, and are deliberately excluded from the window patch artifact. Only enable a new renderer through a reviewed update of this single vendor package.

The vendor trees are the build inputs; patch files are review/export artifacts. Refresh the artifacts from the pinned pristine sources whenever changing a vendor fix. `vendor-checksums.json` records this reviewed snapshot. Never use a hash refresh to conceal an unexplained source change.

## Removing a patch

Verify the upstream release contains the actual fix, update the workspace dependencies, update lockfiles, run the regression and consumer suites, and repeat the affected native-host checks. Then remove the vendor override and its patch artifact, retaining the resolution in this register.

## Native compiler warnings

The Linux release build reports `-Wmaybe-uninitialized` for `oSP`, `low`, and `high` in tuning-library's optional unmapped-reference interpolation path. The `oiko-tuning` parser leaves `allowTuningCenterOnUnmapped` false and rejects unmapped notes. This migration does not change or suppress that upstream code. Investigate it separately before claiming the native dependency builds without warnings.
