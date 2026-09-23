# Wow

[Product page](https://oikoaudio.com/wow/)

> **Beta 0.5.0-beta.1.** These plugins are still in beta, so sound, controls and automation mappings may change between beta releases. Read the [release notes](RELEASE_NOTES.md) before updating existing projects.

**[Download Wow](https://oikoaudio.com/downloads/#wow)** for macOS, Windows, and Linux.

Wow is a free and open-source pitch-modulation plugin for macOS, Windows, and Linux. It continuously changes playback speed to create slow warble, fast flutter, and natural drift. It adds no saturation, hiss, dropouts, or EQ.

Wow reproduces unstable pitch without the unrelated digital artifacts that conventional interpolation adds. Low Amount settings give subtle movement, and high settings give deliberately extreme modulation.

Drag the bottom-right corner to choose a zoom level from 50% to 200% in 25% steps. The selected percentage appears in the center, and the window resizes when you release. The aspect ratio stays fixed. Press Escape during the drag to cancel.

## How Wow reads between samples

To change playback speed, Wow reads audio between its stored samples. A simple interpolator is cheap, but it can lose high frequencies, add unrelated tones, and alias near Nyquist. These artifacts are separate from the sidebands that pitch modulation itself produces.

Wow uses a polyphase windowed-sinc fractional-delay reader. When playback speeds up, the reader's low-pass cutoff follows the instantaneous rate. It removes frequencies that would cross the output Nyquist limit before they can fold back. Wow builds the filter banks outside the audio thread and interpolates smoothly between them during processing. Wow keeps the physically correct FM sidebands and treats any extra interpolation spurs or foldback as errors.

HQ mode is the default. Normal costs less processing than HQ and is less accurate. Ultra costs more and is more accurate. Draft uses a simpler cubic interpolator for comparison and low-cost use.

The difference is most audible on exposed high frequencies and when you layer several modulated signals. The same fractional-delay reader in the shared `oiko-dsp` crate can also serve future modulated-delay and feedback effects.

## Controls

- **Wow Rate** sets the slow oscillator from 0.1 to 4 Hz.
- **Flutter Rate** sets the fast oscillator from 6 to 30 Hz.
- **Wow / Flutter** blends the two oscillators with a constant-power law. The slider shows the Wow share, so 90/10 is 90% full. Moving right adds Wow, and moving left adds Flutter.
- **Amount** sets the total pitch movement.
- **Drift** adds repeatable, smoothly changing variation to both oscillator rates.
- **Phase** rotates both oscillators through a full cycle. The blue pointer shows the common phase offset. Drag the dial or its degree readout, and hold Shift for fine adjustment. The control wraps through zero. Automation follows the shortest circular path with a 20 ms smoothing time constant. Fast phase changes still move pitch, but Wow limits how fast the delay changes during the transition.
- **Stereo spread** separates left and right symmetrically by up to 180 degrees. Its paired-circle symbol sits beneath Phase. Drag its orange degree value or Alt-drag the Phase dial to adjust it, and hold Shift for fine adjustment. The orange arc shows the spread around the phase pointer and wraps across zero. At zero spread the channels stay linked.

Each rate value has an Hz choice and a musical-note choice beside it. They switch that oscillator between free Hz and host-tempo sync, independently of the other oscillator. The active choice is highlighted. In SYNC, each division sits at its equivalent Hz position on the knob. Switching from FREE selects the nearest in-range division. Switching back keeps the division's equivalent Hz speed. Dotted divisions show D and triplets show T. Hover over a synced knob to see its Hz rate.

When the tempo changes, Wow keeps the chosen division while it fits the oscillator's range. Outside that range, Wow plays and displays the nearest in-range division until the chosen division fits again. Sync does not change the depth laws of the Wow / Flutter balance or of either Pitch Range mode.

Synced oscillators anchor their phase to the song beat position in these cases:

- playback starts, loops, or seeks;
- the oscillator's sync division changes during playback;
- sync is turned on during playback.

At each anchor, the oscillator's phase and seeded Drift restart from the beat position, and a running delay moves smoothly onto the new phase. Free-Hz oscillators keep running independently. Modulation continues while the transport is stopped. Drift wanders away from the beat after each anchor. Set Drift to zero to stay on the beat grid once the transition settles. If the host gives no song position, sync follows tempo without re-anchoring. If the host gives no tempo, or a tempo outside 1 to 960 BPM, Wow keeps the last valid tempo, which starts at 120 BPM. New instances and older sessions start with both rates in FREE mode.

The display shows the combined left and right motion of the current settings. Random Seed makes Drift repeat the same way when you reopen a session.

New instances open at 0.6 Hz Wow, 12 Hz Flutter, a 90/10 Wow/Flutter balance, 50% Amount, 50% Drift, and a mono-linked 0° L/R phase offset.

The footer holds two settings you change less often:

- **Quality** selects Draft, Normal, HQ, or Ultra. HQ is the default.
- **Pitch Range** selects Rate-scaled or Constant. Rate-scaled keeps the delay excursion bounded, so faster rates give more pitch movement. Constant keeps the perceived pitch range more even across oscillator rates and needs more latency.

At 48 kHz, Rate-scaled reports about 8.0 ms of latency and Constant reports about 34.2 ms. The host compensates for that latency.

## Formats and platforms

Wow comes as mono and stereo CLAP and VST3 plugins for:

- macOS on Apple Silicon and Intel;
- Windows x86-64;
- Linux x86-64.

The public-beta builds are unsigned and not yet notarized, so expect security warnings from the operating system. Only install an archive downloaded from this repository.

## Installing the beta

Download the archive for your platform from the beta release. Copy the CLAP bundle, the VST3 bundle, or both to the user or system folder for that format:

| Platform | CLAP | VST3 |
|---|---|---|
| macOS | `~/Library/Audio/Plug-Ins/CLAP` | `~/Library/Audio/Plug-Ins/VST3` |
| Windows | `C:\Program Files\Common Files\CLAP` | `C:\Program Files\Common Files\VST3` |
| Linux | `~/.clap` | `~/.vst3` |

After installing, restart the DAW and rescan its plugins. This beta is unsigned, so macOS and Windows may ask you to allow it in the operating system's security settings.

## Beta testing and reports

Reports on host compatibility, automation behaviour, sound at extreme settings, and general usability help most. Please use the [beta report form](https://github.com/oikoaudio/oikoaudio/issues/new?template=bug-report.yml). Include the operating system, DAW and version, plugin format, sample rate, buffer size, and exact steps to reproduce the problem.

Known beta limitations:

- Builds are not signed or notarized.
- Changing Pitch Range changes the reported latency, and the host may restart processing.
- Draft quality is a low-cost audition mode, not the cleanest production setting.
- This beta is still testing which hosts and platforms work.

## Signal quality

Normal, HQ, and Ultra use rate-aware windowed-sinc readers with 80, 96, and 128 taps. Their low-pass cutoff follows the instantaneous playback rate. Around normal speed they keep nearly the full input band, and they suppress frequencies that would otherwise fold below Nyquist. Draft uses a lower-cost cubic interpolator for auditioning and comparison.

## Building

This product is part of the [Oiko Audio workspace](../../README.md). Shared DSP, typography, window handling, and upstream patches live at the repository root. Clone the complete workspace and use the shared `cargo xtask` bundler. See the [engineering principles](../../docs/engineering-principles.md) and [patch register](../../docs/upstream-patches.md).

Install a current stable Rust toolchain, then run from the repository root:

```sh
cargo test --workspace
cargo run -p xtask --release -- bundle wow-plugin --release
```

`Oiko Wow.clap` and `Oiko Wow.vst3` are written to `target/bundled/`.

On macOS, build universal Apple Silicon and Intel CLAP and VST3 bundles with:

```sh
python3 scripts/release.py build wow --platform macOS
```

Releases do not include an Audio Unit build for now. The AUv2 editor passes `auval` but crashes Logic Pro's out-of-process Audio Unit host on macOS 26. The AU packaging project stays in the repository so you can test compatibility once the upstream GUI-hosting path is fixed.

The release workflow tests Wow and creates CLAP and VST3 archives for Linux x86_64, Windows x86_64, and universal macOS on `wow/v*` release tags or manual runs. Only pushed release tags publish a GitHub release. Pull requests run workspace checks. See the [release guide](../../docs/releases.md).

## Repository layout

```text
crates/wow-dsp    Host-independent, real-time DSP core
crates/wow-plugin CLAP/VST3 wrapper and native editor
../../xtask       Shared plugin bundle builder
../../scripts     Shared build and release tooling
```

## License

MIT OR Apache-2.0, at your option. See `LICENSE-MIT` and `LICENSE-APACHE` at the repository root.
