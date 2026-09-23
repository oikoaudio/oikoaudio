# Upstream patch register

The patch artifacts below contain local changes. The resolved upstream section lists fixes that upstream has merged. Edit and test each patch in one place, this repository. The `[patch.crates-io]` section of the root `Cargo.toml` points every consumer at the vendored Rust crates, and `oiko-tuning` builds the vendored tuning-library. Keep upstream notices. Do not create product-specific copies.

Not every local patch fixes an upstream bug. For each change, record whether it fixes a reproduced failure, applies product policy, extends an API, or is an unverified native workaround. A failure reproduced against the recorded base does not show that current upstream main has it.

| Dependency / base | Shared source | Change and upstream disposition |
| --- | --- | --- |
| NicePlug 0.4.0; published crate, Git commit `ce793d8275aebf7589e35fb5fc1e1e3e6a016940` | `vendor/nice-plug` | A local alternative to upstream's state loader. It checks that the stream length fits the platform's size type and grows its buffer fallibly as chunked reads arrive, without upstream's fixed 256 MiB cap for every plugin. A truncated stream or a reported allocation failure fails the load. Large valid envelopes still load. This fix does not cover allocations during deserialization or decompression, or operating-system overcommit. The state-validation preflight is an API extension, not a confirmed upstream bug. |
| NicePlug core 0.4.0; same Git commit | `vendor/nice-plug-core` | Fixes a reproduced parameter-bookkeeping bug. The patch keeps the base and normalized values when modulation clips or a discrete value does not change. It notifies the editor and deduplicates effective-value callbacks. Separately, the default `Plugin::validate_state` hook is an API extension that lets each product validate its own state. |
| NicePlug egui 0.5.0; same Git commit | crates.io | Uses the published adapter. Its Rust source matches the earlier vendored snapshot exactly. No vendor override. |
| egui-baseview 0.7.0 published crate | `vendor/egui-baseview` | For a key event that only presses or releases a modifier, the patch includes that press or release in the reported modifier state. X11 key events report the modifier state from before the event. Other backends are unverified, and nobody has reproduced the problem in a real host. Selective Command/Ctrl shortcut capture is an opt-in API extension. The logging change fixes a reproduced compile failure when both logging backends are disabled. Its gate checks only the tracing backend, so it does not cover diagnostics that use only the log backend. |
| baseview 0.3.2 plus upstream commit `c36ca154f882353f04684973dfe683c1b3d6abb3` | `vendor/baseview` | AppKit tracking-area and cursor fixes that upstream has already merged, including NSZeroRect with NSTrackingInVisibleRect. Wait for a baseview release that contains this commit. Do not propose the same fix upstream again. |
| Surge tuning-library `48422e2f014fcda8dd5d1a4678bb2674faf3bb3e` | `vendor/tuning-library` | A local compatibility extension that accepts a leading minus sign in a KBM reference-note field. Where Inton's Rust code calls the native parser, it validates the supported signed range and the complete result. This is not a confirmed specification bug, so it is not reported upstream. |

## Resolved upstream

Issue 01, where NicePlug applied the first CLAP event before its sample offset, is fixed by upstream [PR #91](https://codeberg.org/RustAudio/nice-plug/pulls/91), commit `41e04a1`. The 0.4.0 version-bump commit includes that fix. There is no local event-loop patch. The repository keeps all eight timing regression tests, and they pass against both the pristine pinned Git sources and the patched vendor sources. Upstream commit `f5235690ce` rescans parameters after a state restore, so there is no local patch for that either. Upstream's typed `VoiceID`, `Channel` and `Key` wildcards replace the local `NOTE_ADDRESS_WILDCARD` constant and the CLAP output conversion patch.

All products use `nice_plug_egui::EguiEditor` directly, with no Oiko host-coordinate wrapper or handle alias. Upstream `NativeSize` defines the host coordinate contract. The separate macOS canvas-zoom workaround still needs native testing.

## Patch artifacts and tests

- `patches/nice-plug-0.4.0.patch`: CLAP wrapper fixes and the state-validation preflight shared by CLAP and VST3. Inton's CLAP host tests cover short stream reads/writes, valid recall, parameter rescanning, malformed state, unrepresentable lengths and truncated input without replacing current state. Run `cargo test -p inton --test clap_host --locked` at the workspace root. The vendored `tests/clap_state_stream.rs` exercises large valid envelopes above 256 MiB, chunk boundaries, partial reads, truncation, invalid read counts, and injected allocation failure through a minimal CLAP host. Its allocator checks that a huge length prefix does not reserve a huge buffer. The test also checks that failed loads keep the parameter values. The vendored `tests/clap_event_timing.rs` uses a minimal plugin and CLAP host to check first-event offsets, automation/modulation, transport boundaries, note forwarding, same-sample groups, block-rate opt-out, and buffer-size independence. `scripts/check_workspace.py --test` runs it through a temporary consumer harness with allocation guards enabled. These timing tests failed against published NicePlug 0.3.0. They pass with the upstream 0.4.0 event loop and no local scheduling change.
- `patches/nice-plug-core-0.4.0.patch`: the matching validation hook and parameter-value fixes. Core regression tests cover upper and lower modulation limits, integer/boolean base edits, quantization, editor notification, and callback deduplication. `scripts/check_workspace.py --test` runs them from a temporary copy. The snapshot includes the ISC notice from the same upstream commit.
- `patches/egui-baseview-0.7.0.patch`: the local input changes and the logging guard. Vendor window tests exercise modifier updates and shortcut routing. Weft and Inton editor tests exercise local undo. Native input behavior is unverified. The published base already has the resize ordering, and no test showed a runtime resize crash caused by logging.
- `patches/tuning-library.patch`: exact header diff against the recorded commit. Run `cargo test -p inton-core --test contract --locked` at the workspace root, including negative-reference validation and tuning fixtures.
- `vendor/baseview` is an unchanged snapshot of the recorded upstream commit, so its upstream Git history is the patch record. Host coordinates use the upstream NicePlug and egui implementation.

## Upgrade validation, 2026-09-09

`python3 scripts/check_workspace.py --test --offline` passed, including warning-denying workspace Clippy, workspace tests, separate product suites, all wrapper harnesses, and core/window vendor regressions. All eight timing cases also passed against pristine NicePlug/core sources at `dc473f2451eecb82b3fd1ab37375ae6ffabc86d3`. Applying both regenerated NicePlug patch artifacts to pristine snapshots reproduced every vendored Rust source and test byte for byte. Native DAW checks were not run. macOS `NativeSize` resizing, lifecycle and the CLAP/VST3 host matrix still need release validation.

## Published release validation, 2026-09-10

`python3 scripts/check_workspace.py --test --offline` passed against the published crate base: formatting, strict Clippy, workspace and individual product suites, CLAP timing/state/note-expression harnesses, and core/window regressions. The egui adapter resolves from crates.io. Native DAW checks were not run.

## Note expression maintenance

The NicePlug patch also fixes the swapped VST3 Brightness and Expression mapping, supplies defaults for each expression type, forwards note-on tuning in semitones, replaces duplicate note-ID cache entries, clears that cache on reset, and forwards ID-only expressions after its bounded channel and key cache wraps. These are local changes. This register does not claim that upstream accepted them or that current upstream main has these bugs. Upstream handles typed wildcard conversion. The CLAP input patch validates the single-port address before narrowing it. The `ClapPlugin::CLAP_SUPPORTS_MPE` flag advertises MPE only for plugins that set it. Plugins that do not set it keep their default dialects. NicePlug core also exports its STFT input traits, so a product can use bounded buffer views without allocating. This export is an API extension.

`vendor/nice-plug/tests/note_expressions.rs` tests the real CLAP entry point and the VST3 expression translator. `scripts/check_workspace.py --test` runs it with the timing and state-stream harnesses. Weft's product tests cover common expression state, active-note changes, same-sample ordering, wildcard/overlap lifecycle, MPE configuration, stereo spectral contributions and process allocation guards. These tests do not show that note expressions route correctly in Bitwig.

## Packaging differences

NicePlug and NicePlug core 0.4.0 come from the published crates.io archives of upstream commit `ce793d8275aebf7589e35fb5fc1e1e3e6a016940`, with the recorded local patches applied on top. The archive SHA-256 checksums match the registry. The vendor trees keep the published manifests and `Cargo.toml.orig`. They leave out the packaged Cargo.lock files because the root lockfile decides the versions. The archives have no LICENSE files, so the vendor trees keep the upstream ISC notices recorded earlier. Both patch artifacts were regenerated against the published Rust sources and tests, and applying them to fresh copies reproduces the vendor trees exactly. NicePlug egui 0.5.0 comes straight from crates.io. Its Rust sources match the earlier vendored snapshot, and it has no vendor override or checksums.

Compared with the earlier snapshot, the published sources add `ParamSetter::request_restart`, flush pending CLAP output parameter events during processing, and replace several host and state debug assertions with warnings. The vendor trees keep these upstream changes along with the local patches.

The egui-baseview vendor snapshot, inherited from Weft and Inton, supports only OpenGL. Its local manifest leaves out the optional wgpu dependencies and examples, and `renderer.rs` selects only OpenGL. These are vendoring choices, not upstream bug fixes, so the window patch artifact leaves them out. To enable another renderer, make a reviewed update to this one vendor package.

Cargo builds from the vendor trees. The patch files exist for review and export. When you change a vendor fix, regenerate its patch artifact from the pinned pristine sources. `vendor-checksums.json` records the hashes of the reviewed vendor trees. Never update the hashes to hide a source change that nobody has explained.

## Removing a patch

1. Verify that the upstream release contains the fix.
2. Update the workspace dependencies and the lockfile.
3. Run the regression and consumer test suites.
4. Repeat the affected native-host checks.
5. Remove the vendor override and its patch artifact.
6. Record the resolution in this register.

## Native compiler warnings

The Linux release build reports `-Wmaybe-uninitialized` for `oSP`, `low`, and `high` in tuning-library's optional unmapped-reference interpolation path. The `oiko-tuning` parser leaves `allowTuningCenterOnUnmapped` false and rejects unmapped notes. This repository does not change or suppress that upstream code. Investigate the warnings before claiming that the native dependency builds without warnings.
