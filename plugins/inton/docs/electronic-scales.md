# Electronic detunings

The factory library has ten original twelve-note tunings. Their largest offsets from 12 EDO range from 6 cents in Soft Circuit to 38 cents in Warped Grid. Find them under Factory, in the category Electronic › Detuned 12-note, or search for `detuned`. All ten keep exact octaves. C and A stay at their 12 EDO pitches, so they match the default mapping root and reference.

## Listening suggestions

**Soft Circuit.** Small fixed offsets for soft pads and quiet synth chords. Try a simple C minor or A minor chord, and compare it with 12 EDO on a sustained sound.

**Loose Clock.** Small irregular offsets for bass sequences and dry plucks. Try a repeating C, Eb, G, Bb pattern.

**Glass Steps.** Lowered thirds next to raised neighboring notes, for glassy arpeggios and synth chords. Try C, E, G, Bb with a bright, harmonically rich patch.

**Bent Fifths.** A narrowed C-G fifth adds tension to familiar chord shapes. Try sustained C-G dyads or a two-note bass ostinato. The fifth is deliberately out of tune. This is not a just intonation scale.

**Split Neon.** Alternating offsets of 18 cents make neighboring steps alternately wider and narrower. Try chromatic plucks or a short repeating sequence. C and A stay fixed as tonal anchors.

**Oxide.** Larger irregular offsets for tense pads, rough stabs and sparse melodies. Try C, Eb, F, G, Bb with long releases to expose the changing intervals.

**Ripple.** The offsets rise in groups and then drop back, so scale runs move in uneven steps. Try ascending semitones on a clean pluck before moving to a longer sequence.

**Warped Grid.** Large offsets in alternating directions, for angular leads and dissonant chord fragments. Start with two or three notes. Octaves stay exact while the intervals inside them change.

**Frayed Thirds.** A nearly pure C-G fifth combines with a widened C-E third and a narrowed C-Eb third. Hold C-G, then add E or Eb to hear the extra beating.

**Restless Fifths.** A nearly pure C-E major third combines with a narrowed C-G fifth. Hold C-E, then add G. Try a sustained saw or pulse sound.

Frayed Thirds has a 408-cent C-E third and a 702-cent C-G fifth. Restless Fifths has a 386-cent third and a 691-cent fifth. Play a sustained C-E-G chord in each. Frayed Thirds beats mostly in the third, and Restless Fifths beats mostly in the fifth. How much beating you hear depends on the register and the patch's harmonics.

## Exact offsets

Each value is the offset in cents from 12 EDO, from C to B:

| Preset | C | C♯ | D | E♭ | E | F | F♯ | G | A♭ | A | B♭ | B |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Soft Circuit | 0 | -4 | +3 | -6 | -3 | +2 | -5 | +1 | -4 | 0 | +4 | -2 |
| Loose Clock | 0 | +8 | -6 | -11 | -7 | +5 | -9 | +2 | +10 | 0 | -8 | +6 |
| Glass Steps | 0 | -12 | +9 | -16 | -14 | +7 | -11 | +2 | +15 | 0 | -18 | +10 |
| Bent Fifths | 0 | +7 | -10 | +12 | -8 | +15 | -14 | -18 | +9 | 0 | -11 | +6 |
| Split Neon | 0 | +18 | -18 | +18 | -18 | +18 | -18 | +18 | -18 | 0 | -18 | +18 |
| Oxide | 0 | -24 | +14 | -31 | -18 | +21 | -27 | +4 | +29 | 0 | -22 | +17 |
| Ripple | 0 | +14 | +28 | -20 | -6 | +8 | +22 | -26 | -12 | 0 | +16 | +30 |
| Warped Grid | 0 | +31 | -27 | +38 | -22 | +26 | -35 | +12 | -29 | 0 | +34 | -18 |
| Frayed Thirds | 0 | -5 | +4 | -8 | +8 | -2 | +7 | +2 | -6 | 0 | +9 | -4 |
| Restless Fifths | 0 | +6 | +2 | -9 | -14 | +10 | -4 | -9 | +7 | 0 | -11 | -3 |

The original Oiko offsets and the generated resources are CC0-1.0. The definitions are in `scripts/electronic-detunings.json`.
