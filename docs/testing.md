# Testing

Run commands from the workspace root. The workspace check covers formatting, strict Clippy, dependency boundaries, product tests and maintained wrapper regressions:

```sh
python3 scripts/check_workspace.py --test --offline
```

## Inton

Build the plugin before running the native checks:

```sh
cargo xtask bundle inton --release --locked --offline
```

The ordinary tests cover tuning and parameter contracts, embedded project recall, malformed state rejection, host parameter rescanning, editing, MTS ownership and allocation-free audio callbacks. Inton validates the NicePlug state envelope and its embedded project before applying either host parameters or project state. Failed loads preserve the current project.

To test the built CLAP without accessing a live MTS master or personal preferences:

```sh
INTON_TEST_CLAP="$PWD/target/bundled/Oiko Inton.clap" \
INTON_MTS_LIBRARY=/nonexistent/inton-state-test.so \
XDG_DATA_HOME=/tmp/inton-state-data XDG_CONFIG_HOME=/tmp/inton-state-config \
  cargo test -p inton --test clap_host --locked --offline
```

The native MTS tests are opt-in and require an idle MTS environment. Close other masters before running them:

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

Repeat editor lifecycle, scaling and state recall checks in the release hosts on each platform, including the maximum 200% zoom.

### DAW acceptance

1. Open Inton in Bitwig and check MTS ownership, including collision and recovery with another master.
2. Assign octave and non-octave scales to slots, then automate Set Position, Morph Time, Reference Frequency and Transpose while playing sustained and new notes.
3. Select or clear an empty slot and verify that the audible tuning is retained.
4. Import an SCL/KBM pair, assign it, save the project and move the source files. Reopen and verify that the embedded tuning is preserved.
5. Check scale browsing, audition, dialog parenting, editor reopen, zoom and multiple instances.
6. Save and reopen with MTS disabled, then verify release and reacquisition.

Receiving instruments must support continuous MTS retuning to glide held notes. Headless tests do not establish that behavior or native host window behavior.
