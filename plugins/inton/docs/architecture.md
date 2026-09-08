# Oiko Inton V1 architecture

The plugin is Rust with a narrow C ABI into the MIT-licensed Surge tuning-library. NICE-PLUG provides the CLAP and VST3 format wrappers, parameter transport, state envelope, transparent audio buffers and note-event conversion. Inton's tuning engine, project model, MTS-ESP worker and editor remain format-independent. The editor uses `nice_plug_egui::EguiEditor` directly, like Oiko Wow and Oiko Weft, with shared `oiko-plugin` editor-state setup and upstream NativeSize host coordinates.

## Thread boundaries

- NICE-PLUG supplies transparent output buffers and converts note events between the host format and its common event model. Inton's parameter callbacks publish independent atomic values to the control worker. The audio callback forwards note events. It performs no file reads, parsing, MTS calls, allocations, or mutex locking. Work is bounded by the host's buffer/event counts and five parameters.
- A single control worker per instance reads the latest parameter values, advances the 128-note log-frequency engine, and publishes via ODDsound's bulk `MTS_SetNoteTunings`. A five-millisecond sleep caps publication at approximately 200 Hz. Identical tables are not republished.
- Project edits and state restore prepare all tuning data outside the audio callback. A session mutex protects the engine, embedded project data, and UI snapshots. It is never acquired by process/flush.
- Library scanning, selected-file reads, and file dialogs run on background jobs. The UI polls completed jobs without blocking on them. Factory data is embedded, and all project playback is independent of indexing and UI lifetime.
- The parameter mailbox separates host updates from control-thread batch edits. A host callback performs one atomic store. Control edits consume pending values before transforming their batch; host updates arriving during the edit remain pending. Readers make one attempt and defer an in-flight control edit. State replacement supersedes earlier updates. No callback acquires the control writer.

## Timing policy

V1 is a real-time source at control rate. Host parameter events are independent updates, including while transport is stopped. Changes faster than a control tick can coalesce. The worker resolves the latest available selection, reference, transpose, and morph duration at its next tick. Morphing uses monotonic wall time, not host sample time, so it works with transport stopped and requires real-time printing of automated sessions. It is not a sample-accurate or faster-than-real-time offline processor.

MTS-ESP's public API offers bulk tables but does not promise realtime safety or a required update rate. Its runtime is a supplied binary supporting IPC. Inton therefore makes no realtime-safety assumption and keeps every library call, acquisition, and release on the worker. The automated native integration test measures complete table publication and verifies all 128 results. Perceived smoothness must also be tested with sustained notes in the receiving instruments; note-on-only clients cannot follow a glide.

## Tuning policy

Surge resolves all SCL intervals and KBM mappings. Original text remains authoritative. Files are capped at 1 MiB and declared counts at 4096 before parsing, with mapped KBM degrees bounded at 4096 before scale expansion; C++ exceptions become Rust error strings. KBM entries must map MIDI 0–127 to finite positive frequencies, with headroom for the exposed global offsets. Unmapped notes are explicitly rejected; there is no invented interpolation through missing pitches.

Prepared tables are calibrated so the mapping's reference MIDI note is 440 Hz; the original mapping frequency is retained as metadata. Reference automation adds `log2(reference/440)` and chromatic transpose adds `semitones/12` to every log-frequency. These offsets shift current/source/target together without restarting a scale morph. New Set targets capture the current interpolated table. Empty targets and clearing the active slot freeze that table.

## Persistence and discovery

NICE-PLUG's versioned state envelope stores the five plain parameters plus Inton's version 1 project field. The project has exactly 32 slots, embedded preset text and metadata, and an optional retained 128-note log table/name. Unknown project versions, malformed data, and incomplete tables fail atomically, leaving the old project intact. Partial CLAP stream reads/writes are supported. Runtime ownership is never serialized, and the same state representation is shared by CLAP and VST3.

User indexing records canonical path, category, size, and modification time of SCL and paired KBM. Cache entries survive rescans only when both file identities are unchanged. Invalid files are cached as errors until a changed file/rescan invalidates them. Factory metadata, aliases, tags, and text are in `resources/library.json`; individual authored SCL resources and the reproducible data generator are included.

## Ownership

The official ODDsound ABI is loaded from its standard platform location: `/usr/local/lib/libMTS.so` on Linux, `/Library/Application Support/MTS-ESP/libMTS.dylib` on macOS, and `%ProgramFiles%\Common Files\MTS-ESP\libMTS.dll` on Windows. `INTON_MTS_LIBRARY` overrides this path for development tests only; production clients resolve the shared library installed by ODDsound. Missing required symbols produce Unavailable. An internal process mutex serializes Inton's acquisition attempts. A busy master is never deregistered or reinitialized. Inton clears stale note filters, disables multi-channel overrides, and invalidates optional mapping metadata when it acquires ownership. Only an owned master is released on shutdown.

## 0.3.0 editing and import changes

Scale editing is a private `scale_edit::Draft`. The model retains original SCL/KBM text when unchanged, resolves changes before assignment, and reuses the existing atomic project-assignment path. Equal-spacing generation resets mapping; ordinary edits preserve it. Cycle extraction remains a developer utility for exceptional import conversion and is not part of the plug-in editor. Draft audition uses the existing temporary publication path and stops on Cancel, Apply, hide or close. Large interval lists are paged twenty entries at a time. Invalid drafts cannot replace a project slot.

The KBM reference lexer accepts a leading minus sign. Rust validates its signed integer range before FFI; the wrapper permits the library's extended reference table (−256…255), while playable output remains MIDI 0…127. Imported pitch tables are never silently reanchored or octave-normalized. Worker failures have a selection-specific preview error, so stale failures cannot turn a newer preview into an indefinite loading state. Parsed user descriptions become searchable without forcing eager parsing of the library.


## Shared foundation and core boundary

`crates/inton-core` owns the engine, validated state, scale-edit draft, and MTS control worker. It uses the shared `oiko-tuning` parser and has no NicePlug or egui dependency. The `inton` crate owns host integration, the editor and library UI, and imports core APIs directly from `inton-core`. Fonts and UI scaling use the shared `oiko-ui` crate. The editor requests scale changes from the About menu through `oiko-ui::scale`, as Wow and Weft do; the window adapter handles host resizing outside UI callbacks. See the root engineering principles and patch register for common contracts and upstream fixes.

Host state runs through `Plugin::validate_state` before NicePlug changes live parameters or persisted fields. The core validates project shape, parameter ranges, retained tables, and native tuning input. Inton uses the applied host parameters when restoring the project so its saved parameter copy cannot override current host values or modulation.
