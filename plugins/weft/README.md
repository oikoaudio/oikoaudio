# Oiko Weft

[Product page](https://oikoaudio.com/weft/)

> **Beta 0.5.0-beta.1.** These plugins are still in beta, so sound, controls and automation mappings may change between beta releases. Read the [release notes](RELEASE_NOTES.md) before updating existing projects.

**[Download Weft](https://oikoaudio.com/downloads/#weft)** for macOS, Windows, and Linux.

Oiko Weft is a spectral mask for CLAP and VST3 hosts. You draw its FFT gain curve and control it with notes. It reshapes audio that already passes through it and does not generate sound by itself.

If you have existing projects, check Motion Shape automation after updating, especially in VST3. Cloud is a new eighth shape, and adding it changes the normalized positions of the shapes. Saved Splash selections now load as Sprinkle. The [compatibility notes](RELEASE_NOTES.md#compatibility-with-earlier-betas) list the affected mappings.

## Quick start

1. Insert Weft as an audio effect on a track or bus that carries sound.
2. Draw the orange curve to set the maximum gain at each frequency.
3. Raise `Note Depth`, then click notes on the pitch ruler or route notes from the host into Weft.
4. Use `Width`, `Partials`, and `Partial Rolloff` to shape the region that each note opens.
5. Optionally, add movement with the Spectral Motion section.

With Note Depth and Motion Depth at 0 dB, a flat orange curve, and Output at 0 dB, Weft passes the input through unchanged after its reported processing latency.

## Audio and note routing

You can supply notes in two ways:

- Click or drag across the pitch ruler to pin notes inside Weft. These notes need no host routing, and Weft saves them with the plugin instance.
- Route note or MIDI events from the host into Weft while audio passes through it.

You can use live and pinned notes together.

Routing depends on the host. In a host with hybrid audio and note tracks, place Weft after the sound source on the same track. In a host with separate routing, send both the audio and the note stream to the track that holds Weft.

`HOLD` pins every live note that is currently down. Notes that arrive later join the same temporary latch. Turn `HOLD` off to release the latch. Notes you clicked on the ruler stay in place. `RESET` beside the ruler clears both sets of pinned notes.

Sounding keys have a blue edge that follows their envelope. A small blue mark shows a pinned note. Hover over a key to preview its fundamental and partials in the graph. The preview follows Partials and Partial Rolloff, and it does not play or select the note.

## Note Control

The orange curve is the base spectral gain. `Depth` lowers frequencies away from active notes and slightly lifts the regions the notes control. The lift rises from 0 to 6 dB over the first 24 dB of Depth and stays at or below 6 dB beyond that. The lift shrinks as Width increases and reaches 0 dB at a Width of 1200 cents. The number of active notes does not change the lift.

At 0 dB Depth, note control has no effect. As Depth increases, Weft acts more and more like a playable spectral gate. The blue curve leaves out the hidden loudness lift, so a fully open note meets the orange ceiling instead of appearing to cross it.

- `Width` sets the frequency spread around each note, in cents.
- `Attack` and `Release` smooth how note regions open and close. The FFT frame time sets a practical lower timing limit even when either is 0 ms. Attack defaults to that minimum, which depends on the resolution. Attack and Release also set each Sprinkle's onset and its automatic decay after the peak. This applies with or without notes and at zero Note Depth. Changes affect only new Sprinkles.
- `Partials` adds up to 24 integer harmonics to every note. A value of 1 uses only the fundamental. For Sprinkle, Partials also sets which harmonics are available, even without MIDI or with Note Depth at zero. Each Sprinkle gesture selects mostly one harmonic, and occasionally two or three.
- `Partial Rolloff` lowers upper note-mask partials by the chosen number of decibels per octave. For Sprinkle, it makes higher harmonics less likely to be selected but applies no extra rolloff gain.
- `VEL SENS` sets velocity sensitivity. At 0%, every note acts at full strength. At 100%, incoming velocity sets note strength directly. Notes clicked on the ruler use a fixed velocity of 50%. You can pin a quieter drone this way and play louder notes over it.
- `BEND ±` sets the pitch-bend range from 1 to 96 semitones.

## Note expression and MPE

Weft accepts note expression whenever the host sends it. There is no MPE on/off switch.

MIDI sustain (CC64) keeps released notes open until you release the pedal on that channel. Notes you still hold keep sounding. The pedal does not affect pinned notes, and a host choke event always ends its matching voice.

| Input | Result |
| --- | --- |
| Per-note tuning or channel pitch bend | Moves the fundamental and all generated partials |
| Per-note pressure or channel pressure | Changes the strength of that note's spectral opening |
| Per-note Gain / volume | Scales the note-opened contribution above the background, including gain above unity |
| Per-note Pan | Places the note contribution in stereo; center preserves the current sound |
| Brightness, Timbre, or MIDI CC74 | Changes Partial Rolloff for that note |
| Per-note vibrato | Adds a 5.5 Hz pitch movement with up to half a semitone of depth |
| Note velocity | Changes note strength according to `VEL SENS` |

With CLAP and VST3 note expression, Weft controls overlapping notes independently if the host supports it. Weft also accepts MIDI pitch bend, pressure and CC74 from MPE controllers.

## Microtuning with MTS-ESP

Weft follows an active MTS-ESP master automatically. Without a master, Weft uses standard tuning and the standard keyboard. Install the MTS-ESP runtime that comes with your tuning software before you start the host.

Live, held and pinned notes follow the master's global tuning table, including changes while notes sound. Pitch bend and note expression add to that tuning. Weft ignores channel-specific MTS tables. Notes that the tuning map excludes do not open spectral regions.

While a master is active, the note ruler shows the scale name and spaces its keys by their actual frequencies. Labels show MIDI note numbers, not octave names. If the master supplies mapping information, shaded boundaries mark repeating key groups. Weft does not assume equal divisions or an octave-based scale. If the scale has duplicate or out-of-order pitches, the ruler uses a MIDI-key layout instead. Pinned notes keep their MIDI note numbers when the master changes or disconnects. Weft's preset does not save the external tuning.

## Drawing and editing the curve

Drag in the graph to edit individual bins exactly. The pointer becomes a small pen with its tip at the draw position. Hold Shift while drawing to use a soft circular pencil, shown by the brush outline. Its centre reaches the pointer value, and neighbouring bins follow with a smooth falloff. Each stroke of either kind is one undo step, recorded when you release the mouse button.

Click `CAPTURE` to start averaging the input spectrum before the mask. Click it again to stop. Weft then normalizes the strongest captured region to 0 dB and writes the result as the base curve. While Capture runs, a faint orange preview shows the average, with the captured audio duration above it. The average counts audio analysis frames, so the display refresh rate does not affect it. A short capture acts like a snapshot, and a longer one gives the stable spectral shape of a track. Capture leaves out Motion and Note Control, which keep working on the result. Press Escape to discard an unfinished capture and keep the existing curve.

Click `FLIP` to turn peaks into valleys within the curve's current vertical range. On a captured curve, the quietest part becomes 0 dB and the strongest part gets the deepest cut. Flip keeps hidden values above and below the graph, and processing still caps the base curve at 0 dB. Flipping twice restores the original shape and level. Flip leaves a flat curve unchanged. Each Flip is one undo step, and Flip is unavailable during Capture.

The `CURVE` menu has Copy, Paste, Save, and Load. These items transfer only the base curve, including hidden values. Motion, notes, output level, and other settings stay as they are. Paste and Load each create one undo step and are unavailable during Capture. Curve files use the `.weftcurve` extension. Files and clipboard data both store the source sample rate, so frequencies line up in another session.

To carve space for one track in another, capture the first track, stop Capture, Flip, then copy the curve. Paste it into Weft on the second track, and use Transform's depth control to set how deep the cut goes. The result is a static spectral cut. It does not isolate or cancel the captured source.

Click `TRANSFORM` to toggle the constrained transform frame. If the host passes the Alt modifier through, you can instead hold Alt while dragging a handle or the graph:

- Drag the softly highlighted bottom edge to change Curve Depth.
- Drag the left or right edge vertically to Tilt around 1 kHz.
- Drag inside the graph horizontally to Shift the curve in semitones.

Weft applies the result to the orange curve when you release the mouse. The frame then returns to neutral, so later drawing uses the normal coordinates. Transform is unavailable while Capture runs. Press Escape to cancel an active drag and leave Transform mode.

If a transform moves part of the curve beyond the visible range, Weft keeps that part of the shape instead of flattening it against the graph edge. Dashed red marks show where the curve continues above or below the graph. Processing caps values above 0 dB at unity gain and turns values at or below -144 dB fully off. A later transform, or drawing in that region, can bring the curve back into view.

The bent-arrow buttons undo and redo these edits:

- parameter adjustments, including Hold and both Reset buttons;
- drawing;
- Capture and Flip;
- transforms;
- curve imports;
- pinned-note edits.

One control drag, one drawing stroke, or one drag across several keys is one step. Switching between Free and Sync, including the rate conversion, is also one step.

Use `Ctrl+Z` and `Ctrl+Shift+Z` on Windows and Linux, or `Cmd+Z` and `Cmd+Shift+Z` on macOS. These shortcuts work anywhere in the focused Weft editor. While you type a numeric value, they edit the text instead. The parameter change enters Weft's history when you commit the value.

Weft's history leaves out live MIDI notes, pedal events, host automation, and display choices such as theme, zoom, and graph range. It is separate from your DAW's project undo history. Weft still reports parameter adjustments to the host as usual.

`RESET` in the graph restores a flat base curve. If Capture is running, Reset cancels it and discards the unfinished average. It leaves the other parameters unchanged.

## Spectral Motion

Spectral Motion reshapes the signal around the orange base curve. `Depth` sets how far the moving field can attenuate the signal. Depth has no effect at 0 dB and reaches 60 dB at its maximum. For the original looping shapes, Weft applies an automatic gain adjustment of at most 12 dB. It reduces the large level jumps that switching between shapes would otherwise cause, especially between Harmonic and Scan.

The factory setting is Drift at 0 dB Depth, synced to 1/2. When you switch from Sync to Free, the Free rate is 0.3 Hz.

- `Ripple` sends smooth repeating waves across the logarithmic spectrum.
- `Harmonic` narrows the waves into a rotating comb.
- `Drift` combines broad waves moving at different rates.
- `Scan` moves one soft open region across the frequency range.
- `Notch` moves a cut across the frequency range.
- `Saw` creates a directional ramp with a short rounded reset.
- `Sprinkle` creates short spectral gestures around played, sustained or pinned notes and their harmonics. Without notes, it picks pitches automatically.
- `Cloud` creates overlapping spectral windows. Notes guide their pitches. Without notes, the windows appear across the whole spectrum. `Direction` changes how they swell and fade.

`Rate` can run freely in hertz or follow the host timeline from 4/1 to 1/64, including dotted and triplet values. The FFT frame rate for the selected Resolution limits very fast settings. `Phase` offsets the cycle. `Size` sets spacing or width in octaves. `Direction` runs forward, reverse, or alternating.

For Sprinkle and Cloud, `Rate` sets activity and `Size` sets width. Size also lengthens Cloud windows. Sprinkle uses the Note Control `Attack` and `Release` to make plucks or swells, and `Partials` and `Partial Rolloff` shape its choice of harmonics. Both modes reveal sound that is already in the input. Set Note Depth to zero to hear them without note gating, and draw the curve to limit their frequency range.

## Resolution, Smooth, and display

`MODE` selects FFT processing at 1024, 2048, 4096, 8192, or 16384 samples. Higher settings give finer frequency control and more latency. Lower settings react faster and make low-frequency bins visibly wider. Resolution changes in steps, and you cannot automate it.

The graph spaces frequencies logarithmically, but FFT bins are evenly spaced in hertz. Hovering shows the exact bin, frequency, nearest note, and cent offset. The small range selector at the top left sets the visible vertical range to 30, 60, 90, or 144 dB. It also sets how deep ordinary drawing can reach at the bottom edge. The bottom of the 144 dB view reads `−∞` because it closes the affected bins completely. Changing the view keeps any parts of the curve that a transform moved beyond the graph.

`SMOOTH` changes the audio processing, not the pencil. It switches to a Blackman analysis window, blends each spectral mask bin with its immediate neighbours, and adds 30 ms of mask smoothing. This can soften abrupt spectral edges, but bins separate less exactly.

A thin peak meter along the top of Output measures the final audio leaving Weft. It turns orange at or above 0 dBFS. The underline below the value still shows the output gain setting. The meter only indicates level and does not limit it.

Click `OIKO AUDIO` in the title bar to open the About panel. The panel sets the interface scale from 50% to 200% in 25% steps. The default is 100%. Click elsewhere or press Escape to close the panel.

Drag the bottom-right corner to choose a zoom level from 50% to 200% in 25% steps. The selected percentage appears in the center, and the window resizes when you release. The aspect ratio stays fixed. Press Escape during the drag to cancel.

## Saved state

The host saves and restores Weft's parameters, curve, pinned ruler notes, display range, and interface scale with the project or with a copied plugin instance. Replacing a development binary while a host is running does not reset an instance that is already loaded. If a host shows stale settings or duplicate plugin entries, see [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md).

## Current beta limitations

- Weft collects note changes once per host audio block and then smooths them at the FFT frame rate.
- Weft does not filter notes by channel.
- Very high-frequency bins can be narrower than one screen pixel. The graph has no horizontal zoom or pan yet.
- Changing Resolution clears the FFT buffers and briefly outputs silence.

## Build from source

This product is part of the [Oiko Audio workspace](../../README.md). Shared DSP, typography, window handling, and upstream patches live at the repository root. Clone the complete workspace and use the shared `cargo xtask` bundler. See the [engineering principles](../../docs/engineering-principles.md) and [patch register](../../docs/upstream-patches.md).

```sh
cargo test --workspace
cargo run -p xtask --release -- bundle spectral-plugin --release
```

The bundler writes the bundles to `target/bundled/`.

On macOS, build universal Apple Silicon and Intel CLAP and VST3 bundles with:

```sh
python3 scripts/release.py build weft --platform macOS
```

Releases do not include an Audio Unit build for now. The AUv2 editor passes `auval` but crashes Logic Pro's out-of-process Audio Unit host on macOS 26. The AU packaging project stays in the repository so you can test compatibility once the upstream GUI-hosting path is fixed.

To generate the deterministic A-minor spectral-mask audition file, run:

```sh
cargo run -p spectral-render --release
```

It writes `target/oiko-spectral-poc.wav` by default.

## Source boundary

Weft contains no GPL-licensed source. It uses NICE-PLUG's public STFT helper, `realfft`, and original Oiko code licensed under MIT OR Apache-2.0.

Each Weft instance registers one MTS-ESP client. The editor reads the processor's tuning snapshot, so opening the editor adds no client. The snapshot uses bounded atomic storage, and the audio thread never allocates or waits for it. For isolated Linux tests, `WEFT_MTS_LIBRARY` overrides the shared-library path. If the override is unavailable, Weft does not fall back to the system master.
