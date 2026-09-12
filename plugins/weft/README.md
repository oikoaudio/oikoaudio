# Oiko Weft

[Product page](https://oikoaudio.com/weft/)

## Shared Rust foundation

This product is part of the [Oiko Audio workspace](../../README.md). Shared DSP, typography, window handling, and upstream patches live at the repository root. Clone the complete workspace and use the common `cargo xtask` bundler. See the [engineering principles](../../docs/engineering-principles.md) and [patch register](../../docs/upstream-patches.md).

> **Beta 0.5.0-beta.1.** These plugins are still maturing; sound, controls and automation mappings may change between beta releases. Read the [release notes](RELEASE_NOTES.md) before updating existing projects.

Oiko Weft is a drawable, note-controlled FFT spectral mask for CLAP and VST3 hosts. It reshapes audio that is already passing through it. It does not generate sound by itself.

Existing projects: check Motion Shape automation after updating, especially in VST3. Adding Cloud changes the normalized positions, and saved Splash selections now use Sprinkle. See the [compatibility notes](RELEASE_NOTES.md#compatibility-with-earlier-betas) for the affected mappings.

## Quick start

1. Insert Weft as an audio effect on a track or bus that carries sound.
2. Draw the orange curve to set the maximum gain at each frequency.
3. Raise `Note Depth`, then click notes on the pitch ruler or route notes from the host into Weft.
4. Use `Width`, `Partials`, and `Partial Rolloff` to shape the regions opened by each note.
5. Add movement with the Spectral Motion section if wanted.

With Note Depth and Motion Depth at 0 dB, a flat orange curve, and Output at 0 dB, Weft passes the input through unchanged after its reported processing latency.

## Audio and note routing

You can supply notes in two ways:

- Click or drag across the pitch ruler to pin notes inside Weft. These notes do not need host routing and are saved with the plug-in instance.
- Route note or MIDI events from the host into Weft while audio passes through it. Live and pinned notes can be used together.

Routing is host-specific. In a host with hybrid audio and note tracks, place Weft after the sound source on the same track. In a host with separate routing, send both the audio and the note stream to the track that contains Weft.

`HOLD` pins every live note that is currently down, then adds later incoming notes to the same temporary latch. Turn `HOLD` off to release that latch. Manually clicked notes remain in place. `RESET` beside the ruler clears both sets of pinned notes.

Sounding keys have a blue edge that follows their envelope. A small blue mark identifies a pinned note. Hover a key to preview its fundamental and partials in the graph; the preview follows Partials and Partial Rolloff without playing or selecting the note.

## Note Control

The orange curve is the base spectral gain. `Depth` lowers frequencies away from active notes and gently lifts the note-controlled regions. The lift grows from 0 to 6 dB through the first 24 dB of Depth, then stays bounded. It tapers as Width increases and reaches 0 dB at a Width of 1200 cents. This compensation does not depend on the number of active notes.

At 0 dB Depth, note control is neutral. Increasing Depth turns the effect into a progressively stronger playable spectral gate. The blue curve omits the hidden loudness compensation, so a fully open note meets the orange ceiling instead of appearing to break through it.

- `Width` sets the frequency spread around each note, in cents.
- `Attack` and `Release` smooth the opening and closing of note regions. The FFT frame time still sets a practical lower timing limit when either is 0 ms. Attack defaults to that resolution-dependent minimum. These controls also set each Sprinkle's onset and automatic decay after its peak, with or without notes and at zero Note Depth; edits affect new Sprinkles.
- `Partials` adds up to 24 integer harmonics for every note. A value of 1 uses only the fundamental. For Sprinkle, this also sets the available harmonics, even without MIDI or with Note Depth at zero; each gesture selects mostly one, occasionally two or three.
- `Partial Rolloff` reduces upper note-mask partials by the chosen number of decibels per octave. For Sprinkle, it also makes higher harmonics less likely to be selected, without applying another rolloff gain.
- `VEL SENS` sets velocity sensitivity. At 0%, every note acts at full strength. At 100%, incoming velocity directly controls note strength. Notes clicked on the ruler use a fixed velocity of 50%, which makes it possible to pin a quieter drone and play louder notes over it.
- `BEND ±` sets the pitch-bend range from 1 to 96 semitones.

## Note expression and MPE

Weft accepts note expression whenever the host supplies it. There is no MPE on/off switch.

MIDI sustain (CC64) keeps released notes open until the pedal is released on that channel. Notes still physically held continue sounding. The pedal does not change pinned notes, and a host choke event always ends its matching voice.

| Input | Result |
| --- | --- |
| Per-note tuning or channel pitch bend | Moves the fundamental and all generated partials |
| Per-note pressure or channel pressure | Changes the strength of that note's spectral opening |
| Per-note Gain / volume | Scales the note-opened contribution above the background, including gain above unity |
| Per-note Pan | Places the note contribution in stereo; center preserves the current sound |
| Brightness, Timbre, or MIDI CC74 | Changes Partial Rolloff for that note |
| Per-note vibrato | Adds a 5.5 Hz pitch movement with up to half a semitone of depth |
| Note velocity | Changes note strength according to `VEL` |

CLAP and VST3 note expression can control overlapping notes independently when the host supports it. Weft also accepts MIDI pitch bend, pressure and CC74 from MPE controllers.

## Microtuning with MTS-ESP

Weft follows an active MTS-ESP master automatically. Without one, tuning and the keyboard work as before. Install the MTS-ESP runtime supplied with your tuning software before starting the host.

Live, held and pinned notes follow the master's global tuning table, including changes while notes are sounding. Pitch bend and note expression add to that tuning. Channel-specific MTS tables are not used. Notes excluded by the tuning map do not open spectral regions.

While a master is active, the note ruler shows the scale name and spaces its keys by their actual frequencies. Labels identify MIDI note numbers rather than conventional octave names. Where the master supplies mapping information, shaded boundaries mark repeating key groups. This does not assume equal divisions or an octave-based scale. Duplicate or out-of-order pitches use a MIDI-key layout instead. Pinned notes keep their MIDI identities when the master changes or disconnects; the external tuning itself is not saved in Weft's preset.

## Drawing and editing the curve

Drag in the graph for exact per-bin editing. The pointer becomes a small pen with its tip at the draw position. Hold Shift while drawing to use a soft circular pencil, indicated by the brush outline. Its centre reaches the pointer value while neighbouring bins follow with a smooth falloff. Both kinds of stroke create one undo step when the mouse button is released.

Click `CAPTURE` to begin averaging the pre-mask input spectrum. Click it again to stop, normalize the strongest captured region to 0 dB, and write the result as the base curve. A faint orange preview shows the average while Capture runs, with the accumulated audio duration above it. The average follows audio analysis frames, so display refresh speed does not affect it. A short capture behaves like a snapshot. A longer one describes the stable spectral shape of a track. Motion and Note Control are not included in the capture and continue to operate on the result. Press Escape to discard an unfinished capture without changing the existing curve.

Click `FLIP` to turn peaks into valleys across the curve's existing vertical range. For a captured curve, its quietest part becomes 0 dB and its strongest part receives the deepest cut. Flip preserves hidden values above and below the graph; processing still caps the base curve at 0 dB. Flip twice returns the original shape and level. A flat curve is unchanged. Each Flip is one undo step, and the operation is unavailable during Capture.

The `CURVE` menu provides Copy, Paste, Save, and Load. These transfer only the base curve, including hidden values. Motion, notes, output level, and other settings stay as they are. Paste and Load each create one undo step and are unavailable during Capture. Curve files use the `.weftcurve` extension. Both files and clipboard data include the source sample rate so frequencies match when transferring to another session.

To carve space for one track in another, capture the first track, stop Capture, Flip, then Copy the curve. Paste it into Weft on the second track and use Transform's depth control to adjust the amount of cutting. This creates a static spectral cut; it does not isolate or cancel the captured source.

Click `TRANSFORM` to toggle the constrained transform frame. Alternatively, hold Alt while dragging a handle or the graph when the host passes that modifier through:

- Drag the softly highlighted bottom edge to change Curve Depth.
- Drag the left or right edge vertically to Tilt around 1 kHz.
- Drag inside the graph horizontally to Shift the curve in semitones.

The result is applied to the orange curve when you release the mouse. The frame then returns to neutral, so later drawing works in the normal coordinate space. Transform is unavailable while Capture is running. Press Escape to cancel an active drag and leave Transform mode.

If a transform moves part of the curve beyond the visible range, Weft keeps that part of the shape instead of flattening it against the graph edge. Dashed red marks show where it continues above or below the graph. Processing caps values above 0 dB at unity gain and treats values at or below -144 dB as fully off. A later transform, or drawing in that region, can bring the curve back into view.

The bent-arrow buttons undo and redo parameter adjustments, drawing, capture, Flip, transforms, curve imports, and pinned-note edits. Hold and both Reset buttons are included. One control drag, drawing stroke, or drag across several keys is one step. Switching between Free and Sync, including the rate conversion, is also one step.

Use `Ctrl+Z` and `Ctrl+Shift+Z` on Windows and Linux, or `Cmd+Z` and `Cmd+Shift+Z` on macOS. These shortcuts work throughout the focused Weft editor. While typing a numeric value, they edit the text instead; the parameter change enters Weft's history when you commit it.

Live MIDI notes, pedal events, host automation, and display choices such as theme, zoom, and graph range do not enter this history. Weft's local history is separate from your DAW's project undo history. Parameter adjustments are still reported to the host normally.

`RESET` in the graph restores a flat base curve. If Capture is running, Reset cancels it and discards the unfinished average. It does not reset the other parameters.

## Spectral Motion

Spectral Motion reshapes the signal around the orange base curve. `Depth` controls how far the moving field can attenuate it. For the original looping shapes, a bounded automatic gain adjustment reduces the large level changes that would otherwise occur when switching between shapes, especially Harmonic and Scan. It can add no more than 12 dB. Depth is neutral at 0 dB and reaches 60 dB at its maximum.

The factory setting is Drift at 0 dB Depth, synchronized to 1/2. The Free rate is set to 0.3 Hz when you switch from Sync to Free.

- `Ripple` sends smooth repeating waves across the logarithmic spectrum.
- `Harmonic` narrows the waves into a rotating comb.
- `Drift` combines broad waves moving at different rates.
- `Scan` carries one soft open region across the frequency range.
- `Notch` moves a cut across the frequency range.
- `Saw` creates a directional ramp with a short rounded reset.
- `Sprinkle` creates short spectral gestures around played, sustained or pinned notes and their harmonics. Without notes, it chooses pitches automatically.
- `Cloud` creates overlapping spectral windows. Notes guide their pitches; without notes, they appear across the spectrum. `Direction` changes how they swell and fade.

`Rate` can run freely in hertz or follow the host timeline from 4/1 through 1/64, including dotted and triplet values. Very fast settings are limited by the FFT frame rate for the selected Resolution. `Phase` offsets the cycle. `Size` controls spacing or width in octaves. `Direction` can run forward, reverse, or alternate.

For Sprinkle and Cloud, `Rate` controls activity and `Size` controls width; Size also lengthens Cloud windows. Sprinkle uses Note Control `Attack` and `Release` for plucks or swells, and `Partials` and `Partial Rolloff` shape its harmonic choices. Both modes reveal sound already in the input. Try Note Depth at zero to hear them without note gating, and use the drawn curve to control the frequency range.

## Resolution, Smooth, and display

`MODE` selects 1024, 2048, 4096, 8192, or 16384 sample FFT processing. Higher settings give finer frequency control and more latency. Lower settings react faster and make low-frequency bins visibly wider. Resolution is stepped and cannot be automated.

The graph uses logarithmic frequency spacing, but FFT bins are equally spaced in hertz. Hovering shows the exact bin, frequency, nearest note, and cent offset. The small range selector at the top left changes the visible vertical range between 30, 60, 90, and 144 dB. It also sets how deeply ordinary drawing can reach at the bottom edge. The bottom of the 144 dB view is labelled `−∞` because it closes the affected bins completely. Changing the view does not discard parts of a curve that a transform has moved beyond the graph.

`SMOOTH` changes the audio processing, not the pencil. It uses a Blackman analysis window, blends each spectral mask bin with its immediate neighbours, and adds 30 ms of mask smoothing. This can soften abrupt spectral edges at the cost of less exact bin separation.

A thin peak meter along the top of Output measures the final audio leaving Weft. It turns orange at or above 0 dBFS. The underline below the value still shows the output gain setting. The meter is an indicator, not a limiter.

Click `OIKO AUDIO` in the title bar to open the About panel. Interface scale is available there from 50% to 200% in fixed 25% steps. The default is 100%. Click elsewhere or press Escape to close the panel.

Drag the bottom-right corner to choose a zoom level from 50% to 200% in 25% steps. The selected percentage appears in the center; release to resize the window. The aspect ratio stays fixed. Press Escape during the drag to cancel.

## Saved state

The host saves and restores Weft's parameters, curve, pinned ruler notes, display range, and interface scale with the project or copied plug-in instance. Replacing a development binary while a host is running does not reset an already loaded instance. See [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md) if a host shows stale settings or duplicate plug-in entries.

## Current beta limitations

- Note changes are collected once per host audio block, then smoothed at the FFT frame rate.
- Note-channel filtering is not implemented.
- Very high-frequency bins can be narrower than one screen pixel. The graph does not yet have horizontal zoom or pan.
- Changing Resolution clears the FFT buffers and briefly outputs silence.
- Bitwig Studio 6.1 may not refresh reported CLAP latency after a Resolution change. The VST3 build is not affected. See [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md).

## Build from source

```sh
cargo test --workspace
cargo run -p xtask --release -- bundle spectral-plugin --release
```

Bundles are written to `target/bundled/`.

On macOS, build universal Apple Silicon and Intel CLAP and VST3 bundles with:

```sh
python3 scripts/release.py build weft --platform macOS
```

Audio Unit distribution is temporarily disabled because the AUv2 editor crashes Logic Pro's out-of-process Audio Unit host on macOS 26 despite passing `auval`. The AU packaging project remains in the repository for compatibility testing after the upstream GUI hosting path is fixed.

Generate the deterministic A-minor spectral-mask audition file with:

```sh
cargo run -p spectral-render --release
```

The default output is `target/oiko-spectral-poc.wav`.

## Source boundary

Weft contains no GPL-licensed source. It uses NICE-PLUG's public STFT helper, `realfft`, and original MIT-licensed Oiko code.

MTS-ESP uses one client registration per Weft instance. The editor reads the processor’s tuning snapshot; opening the editor adds no client. The snapshot uses bounded atomic storage, with no audio-thread allocation or waiting. For isolated Linux tests, `WEFT_MTS_LIBRARY` overrides the shared-library path; an unavailable override does not fall back to the system master.
