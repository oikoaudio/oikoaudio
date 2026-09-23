# Testing

Run these commands from the workspace root. The workspace check runs formatting, strict Clippy, the dependency boundary checks, the product tests and the regression tests for the patched NicePlug wrappers:

```sh
python3 scripts/check_workspace.py --test --offline
```

## Inton

Build the plugin before running the native checks:

```sh
cargo xtask bundle inton --release --locked --offline
```

The regular tests cover tuning and parameter contracts, embedded project recall, rejection of malformed state, host parameter rescanning, editing, MTS ownership and allocation-free audio callbacks. Inton validates the NicePlug state envelope and its embedded project before it applies either the host parameters or the project state. A failed load leaves the current project unchanged.

This command tests the built CLAP without connecting to a live MTS master or reading your preferences:

```sh
INTON_TEST_CLAP="$PWD/target/bundled/Oiko Inton.clap" \
INTON_MTS_LIBRARY=/nonexistent/inton-state-test.so \
XDG_DATA_HOME=/tmp/inton-state-data XDG_CONFIG_HOME=/tmp/inton-state-config \
  cargo test -p inton --test clap_host --locked --offline
```

The native MTS tests run only with `--ignored`. They need an MTS environment with no other master, so close other MTS masters first:

```sh
INTON_MTS_LIBRARY="$PWD/plugins/inton/vendor/mts-esp/libMTS/Linux/x86_64/libMTS.so" \
  cargo test -p inton --test native_mts --locked --offline -- --ignored

INTON_TEST_CLAP="$PWD/target/bundled/Oiko Inton.clap" \
INTON_MTS_LIBRARY="$PWD/plugins/inton/vendor/mts-esp/libMTS/Linux/x86_64/libMTS.so" \
  cargo test -p inton --test clap_host --locked --offline built_clap_automates_official_mts_without_an_editor -- --ignored
```

With an X11 display, the editor smoke test opens and closes its own windows with MTS disabled:

```sh
INTON_MTS_LIBRARY=/nonexistent/inton-ui-test.so \
XDG_DATA_HOME=/tmp/inton-ui-data XDG_CONFIG_HOME=/tmp/inton-ui-config \
  cargo run -p inton --example gui_probe --locked --offline -- "$PWD/target/bundled/Oiko Inton.clap"
```

Repeat the editor lifecycle, scaling and state recall checks in the release hosts on each platform, including at the maximum zoom of 200%.

### DAW acceptance

1. Open Inton in Bitwig and check that it becomes the MTS-ESP master. Open a second master and check that Inton reports it is not sending tuning. Close the other master, click Reset and reconnect, and check that Inton sends its scale again. Disable and re-enable the receiving instruments, then check the client count.
2. Assign octave and non-octave scales to slots, then automate Set Position, Morph Time, Reference Frequency and Transpose while playing sustained and new notes.
3. Select or clear an empty slot and verify that the audible tuning does not change.
4. Import an SCL/KBM pair, assign it, save the project and move the source files. Reopen the project and verify that it still has the embedded tuning.
5. Check scale browsing, audition, dialog parenting, editor reopen, zoom and multiple instances.
6. Turn MTS off, save and reopen the project, and verify that Inton does not hold the master role. Turn MTS on again and verify that Inton takes the role back.

Held notes glide only in receiving instruments that support continuous MTS retuning. Headless tests cannot check that, and they cannot check native host window behavior.
