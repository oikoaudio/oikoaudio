# Upstream patch register

All source changes below are local. No pull requests have been opened. Edit and test a patch once in this repository. The root workspace patches all consumers to these sources. Retain upstream notices. Do not create product-specific copies.

A local patch is not necessarily an upstream bug fix. Distinguish reproduced failures from product policy, API extensions, and unverified native workarounds. Reproduction against a recorded base does not establish that current upstream main is affected.

| Dependency / base | Shared source | Change and upstream disposition |
| --- | --- | --- |
| NicePlug 0.3.0; crate commit `4450436d9f914aec780d17cea19f18d90fa863c9` | `vendor/nice-plug` | Local CLAP state reader cap of 256 MiB with checked platform conversion; parameter-value rescan after successful restore; common state-validation preflight before changing parameters or fields. The size cap is local policy, and the validation hook is an API extension. |
| NicePlug core 0.3.0; crate commit `4450436d9f914aec780d17cea19f18d90fa863c9` | `vendor/nice-plug-core` | Reproduced parameter-bookkeeping bug: preserve base and normalized values when modulation is clipped or a discrete value does not change, and notify the editor while deduplicating effective-value callbacks. Separately, the default Plugin::validate_state hook is an API extension for product-owned validation. |
| egui-baseview 0.7.0 published crate | `vendor/egui-baseview` | Modifier-only state adjustment is a suspected native bug workaround; backend semantics and real-host reproduction remain unverified. Selective Command/Ctrl shortcut capture is an opt-in API extension. Published 0.7.0 already services resize commands after the UI callback; that ordering is not our fix. The logging delta addresses a reproduced compile failure with both logging backends disabled. Local tracing-only gating does not cover log-only diagnostics; a revised guard has only been tested in an investigation copy. |
| baseview 0.3.2 plus upstream commit `c36ca154f882353f04684973dfe683c1b3d6abb3` | `vendor/baseview` | Existing upstream AppKit tracking-area and cursor fixes, including NSZeroRect with NSTrackingInVisibleRect. Already upstream: wait for a release containing this commit; do not propose the same fix again. |
| Surge tuning-library `48422e2f014fcda8dd5d1a4678bb2674faf3bb3e` | `vendor/tuning-library` | Local compatibility extension accepting a leading minus sign in a KBM reference-note field. Inton's Rust/native boundary validates the supported signed range and the complete result. Not a confirmed specification bug; excluded from upstream reporting. |
| NicePlug/egui host-coordinate adapter | `crates/oiko-plugin/src/host_coordinates.rs` | Oiko wrapper correcting AppKit point/pixel size exchange. Not a vendored source modification. Needs macOS host reproducer and agreement on the upstream coordinate contract before proposing a fix. |

## Patch artifacts and tests

- `patches/nice-plug-0.3.0.patch`: CLAP state stream checks, host rescanning and common CLAP/VST3 state-validation preflight. Product host regressions accompany the consuming plugins when they are added to the workspace.
- `patches/nice-plug-core-0.3.0.patch`: the matching validation hook and parameter-value fixes. Core regression tests cover upper and lower modulation limits, integer/boolean base edits, quantization, editor notification, and callback deduplication. `scripts/check_workspace.py --test` runs them from a temporary copy. The published core crate omits a license file; its snapshot includes the ISC notice from NicePlug at the same upstream commit.
- `patches/egui-baseview-0.7.0.patch`: local input behavior and logging guard. Vendor window tests exercise modifier updates and shortcut routing; Weft and Inton editor tests exercise local undo. Native input behavior remains unverified. Resize ordering already exists in the published base; no logging-induced runtime resize crash was demonstrated.
- `patches/tuning-library.patch`: exact header diff against the recorded commit. Run `cargo test -p inton-core --test contract --locked` at the workspace root, including negative-reference validation and tuning fixtures.
- baseview is an unchanged snapshot of the recorded upstream commit. Its upstream Git history is the patch record; the local coordinate wrapper is reviewed separately.

## Packaging differences

The egui-baseview vendor snapshot inherited from Weft/Inton is OpenGL-only: its local manifest omits optional wgpu dependencies/examples and renderer.rs selects only OpenGL. These are vendoring choices, not upstream bug fixes, and are deliberately excluded from the window patch artifact. Only enable a new renderer through a reviewed update of this single vendor package.

The vendor trees are the build inputs; patch files are review/export artifacts. Refresh the artifacts from the pinned pristine sources whenever changing a vendor fix. `vendor-checksums.json` records this reviewed snapshot. Never use a hash refresh to conceal an unexplained source change.

## Removing a patch

Verify the upstream release contains the actual fix, update the workspace dependencies, update lockfiles, run the regression and consumer suites, and repeat the affected native-host checks. Then remove the vendor override and its patch artifact, retaining the resolution in this register.

## Native compiler warnings

The Linux release build reports `-Wmaybe-uninitialized` for `oSP`, `low`, and `high` in tuning-library's optional unmapped-reference interpolation path. The `oiko-tuning` parser leaves `allowTuningCenterOnUnmapped` false and rejects unmapped notes. This migration does not change or suppress that upstream code. Investigate it separately before claiming the native dependency builds without warnings.
