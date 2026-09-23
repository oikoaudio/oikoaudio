# Oiko Inton architecture

Inton is written in Rust and calls the MIT-licensed Surge tuning-library through a narrow C ABI. NicePlug provides the CLAP and VST3 format wrappers, parameter transport, the state envelope, transparent audio buffers and note-event conversion. Inton's tuning engine, project model, MTS-ESP worker and editor do not depend on the plugin format. Like Oiko Wow and Oiko Weft, the editor uses `nice_plug_egui::EguiEditor` directly. It gets its editor-state setup from the shared `oiko-plugin` crate and uses upstream NativeSize host coordinates.

## Thread boundaries

- NicePlug supplies transparent output buffers and converts note events between the host format and its common event model. Inton's parameter callbacks publish independent atomic values to the control worker. The audio callback forwards note events. It does not read files, parse, call MTS, allocate or lock a mutex. Its work is bounded by the host's buffer and event counts and by the five parameters.
- Each instance has one control worker. The worker reads the latest parameter values, advances the 128-note log-frequency engine and publishes the table through ODDSound's bulk `MTS_SetNoteTunings`. The worker sleeps five milliseconds between ticks, which limits publication to about 200 Hz. It skips publication when the table has not changed.
- Project edits and state restore prepare all tuning data outside the audio callback. A session mutex protects the engine, embedded project data and UI snapshots. The process and flush callbacks never acquire it.
- Library scanning, selected-file reads and file dialogs run as background jobs. The UI polls for completed jobs and never blocks on them. Factory data is embedded in the plugin, and project playback does not depend on indexing or on whether the UI is open.
- The parameter mailbox keeps host updates separate from batch edits made on the control thread. A host callback performs one atomic store. A control edit takes the pending values before it transforms its batch. Host updates that arrive during the edit stay pending. A reader makes one attempt and defers to a control edit in progress. State replacement discards earlier pending updates. No callback acquires the control writer.

## Timing policy

Inton sends tuning in real time at control rate. It treats host parameter events as independent updates, including while transport is stopped. Changes that arrive faster than one control tick can merge. At each tick the worker applies the latest selection, reference, transpose and morph duration. Morphing follows monotonic wall-clock time, not host sample time. Morphs therefore work with transport stopped, and automated sessions must be printed in real time. Inton is not sample-accurate and cannot render offline faster than real time.

MTS-ESP's public API offers bulk tables, but it does not promise realtime safety or require an update rate. Its runtime is a supplied binary that uses IPC. Inton therefore assumes no realtime safety and makes every library call, acquisition and release on the worker. The automated native integration test measures publication of a complete table and checks all 128 results. Smooth glides still need a listening test with sustained notes in the receiving instruments. Clients that read tuning only at note-on cannot follow a glide.

## Tuning policy

Surge resolves all SCL intervals and KBM mappings. The original file text remains the source of truth. Before parsing, Inton rejects files larger than 1 MiB and declared counts above 4096. It also limits mapped KBM degrees to 4096 before scale expansion. C++ exceptions become Rust error strings. KBM entries must map MIDI 0 to 127 to finite positive frequencies, with headroom for the global reference and transpose offsets. Inton rejects unmapped notes and never interpolates pitches for missing keys.

Prepared tables are calibrated so the mapping's reference MIDI note is 440 Hz. The original mapping frequency is kept as metadata. Reference automation adds `log2(reference/440)` to every log-frequency, and chromatic transpose adds `semitones/12`. These offsets shift the current, source and target tables together without restarting a scale morph. A new Set target starts from the current interpolated table. Selecting an empty target or clearing the active slot freezes that table.

## Persistence and discovery

NicePlug's versioned state envelope stores the five plain parameters and Inton's version 1 project field. The project has exactly 32 slots, embedded preset text and metadata, and an optional retained 128-note log table with its name. An unknown project version, malformed data or an incomplete table fails the whole restore and leaves the old project intact. Inton supports partial CLAP stream reads and writes. It never serializes runtime ownership, and CLAP and VST3 share one state representation.

User indexing records the canonical path, category, size and modification time of each SCL file and its paired KBM file. A cache entry survives a rescan only when neither file has changed. Invalid files stay cached as errors until the file changes or a rescan invalidates the entry. Factory metadata, aliases, tags and text are in `resources/library.json`. The repository includes each authored SCL resource and the script that regenerates the data.

## Ownership

Inton loads the official ODDSound ABI from its standard platform location: `/usr/local/lib/libMTS.so` on Linux, `/Library/Application Support/MTS-ESP/libMTS.dylib` on macOS, and `%ProgramFiles%\Common Files\MTS-ESP\libMTS.dll` on Windows. `INTON_MTS_LIBRARY` overrides this path for development tests only. Production clients load the shared library that ODDSound installs. If required symbols are missing, the status is Unavailable. An internal process mutex serializes Inton's acquisition attempts. Inton never deregisters or reinitializes a busy master that another program holds. When Inton acquires ownership, it clears stale note filters, disables multi-channel overrides and invalidates optional mapping metadata. On shutdown it releases the master only if it owns it.

## Scale editing and import

Scale editing works on a private `scale_edit::Draft`. The draft keeps the original SCL and KBM text while it is unchanged. It resolves changes before assignment and assigns the result through the same atomic project-assignment path as other loads. Equal-spacing generation resets the mapping, and other edits keep it. Cycle extraction (`Draft::extract_cycle`) is a developer utility for unusual import conversions and is not in the plugin editor. Draft audition uses the same temporary publication path as Library audition. It stops on Cancel, Apply, hide or close. The editor shows long interval lists in pages of twenty entries. An invalid draft cannot replace a project slot.

The KBM reference lexer accepts a leading minus sign. Rust checks the signed integer range before the FFI call. The wrapper allows the library's extended reference table from −256 to 255, while playable output stays within MIDI 0 to 127. Inton never re-anchors or octave-normalizes imported pitch tables. Each worker failure carries a preview error for its own selection, so an old failure cannot leave a newer preview loading forever. Parsed user descriptions become searchable, and the library does not parse every file up front to make that possible.

## Shared foundation and core boundary

`crates/inton-core` owns the engine, validated state, scale-edit draft and MTS control worker. It uses the shared `oiko-tuning` parser and does not depend on NicePlug or egui. The `inton` crate owns host integration and the editor and library UI, and it imports core APIs directly from `inton-core`. Fonts and UI scaling come from the shared `oiko-ui` crate. As in Wow and Weft, the editor requests scale changes from the About menu through `oiko-ui::scale`. The window adapter handles host resizing outside UI callbacks. The root engineering principles and patch register describe the common contracts and upstream fixes.

`Plugin::validate_state` checks host state before NicePlug changes live parameters or persisted fields. The core validates the project shape, parameter ranges, retained tables and native tuning input. When restoring the project, Inton uses the host parameters that were applied. Its saved copy of the parameters therefore cannot override current host values or modulation.
