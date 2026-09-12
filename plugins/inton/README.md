# Oiko Inton

[Product page](https://oikoaudio.com/inton/)

## Shared Rust foundation

This product is part of the [Oiko Audio workspace](../../README.md). Shared DSP, typography, window handling, and upstream patches live at the repository root. Clone the complete workspace and use the common `cargo xtask` bundler. See the [engineering principles](../../docs/engineering-principles.md) and [patch register](../../docs/upstream-patches.md).


> **Beta 0.5.0-beta.1.** These plugins are still maturing; sound, controls and automation mappings may change between beta releases. Read the [release notes](RELEASE_NOTES.md) before updating existing projects.

Oiko Inton tunes MTS-ESP instruments from one place in your DAW. Choose a scale for the whole track, or build a Scale set and automate tuning changes. Your DAW project saves the scales with Inton.

Inton is a CLAP and VST3 plugin for Linux, Windows and macOS.

## Start playing

1. Insert Inton in your DAW. It starts with 12 EDO, the usual twelve equally spaced notes per octave.
2. Open an instrument that supports MTS-ESP and enable its MTS tuning option if needed. Play notes through that instrument. Inton does not make sound itself.
3. Click the folder icon to open the Library. With one scale loaded, clicking a scale in the list changes the tuning immediately.
4. Click the folder icon again, or press Escape, to close the Library.

To hear the differences clearly, try chords with a sawtooth patch. Small tuning changes can be hard to hear with low sine tones. Use 12 EDO as a reference.

Drag the bottom-right corner to choose a zoom level from 50% to 200% in 25% steps. The selected percentage appears in the center; release to resize the window. The aspect ratio stays fixed. Press Escape during the drag to cancel.

## Find a scale

Opening the Library selects the current scale and focuses search. Type to search names, categories, tags and descriptions. If an existing filter hides the current scale, opening the Library clears that filter.

The star filters Favorites, the factory icon filters factory scales, and the person icon filters your files. Click the selected filter again to show all sources. Click a star beside a scale to change your Favorites.

The funnel opens the category filter. Choose Listening guide for eight scales with listening exercises. This collection stays available when you change your Favorites. Click Playing ideas below the scale information to read a suggestion. The button appears in both the main view and the browser when the scale has guidance.

The arrows beside the tabs step through the filtered Library. Up and Down on the keyboard move from search into the list and continue from the selected scale. With one scale loaded, these choices change the current tuning. When adding to or arranging a Scale set, they select a scale for inspection instead.

The factory library contains 49 tunings, including electronic detunings, equal divisions and Carlos Alpha, Beta and Gamma. Your own scale files appear alongside them.

## Use several scales

The list icon opens Scale set. Its numbered slots let you prepare tuning changes for a track. You can use up to 32 slots.

Drag a Library scale onto the Scale set tab to append it. The tab flashes when it accepts the scale. This does not activate the new scale or switch tabs. Open Scale set to see it.

Click the dashed + row to browse for another scale. In this mode, select a Library scale and click Add, double-click it, or press Right to add it. The speaker icon lets you hear a temporary audition first. Orange means audition is on.

Click a slot's dot or double-click its row to activate it. The orange dot marks the active slot. A single click on the row lets you inspect it. The arrows beside the tabs select the previous or next occupied slot, skipping gaps. The number to their right is Set position, the same position your DAW can automate.

To replace an occupied slot, click its circular-arrows button, then choose a Library scale and load it. Escape cancels replacement. To fill an empty slot, select it before loading a scale. You can also drag a Library scale onto any slot to replace its contents.

Drag a populated slot to reorder the scales. The insertion line shows where it will go. The active scale follows its new position.

## Clear slots and undo changes

The × on a slot clears its scale. If occupied slots follow it, the empty slot stays so their automation numbers do not change. If no occupied slots follow it, Inton removes that row. Adding a scale with + uses a new slot rather than filling a gap.

Deleting the active scale activates the first remaining scale. If no scales remain, Inton holds the last tuning. Choosing an empty or unused position through automation also holds the current tuning. It does not create more rows.

The trash button at the bottom of Scale set clears the set and keeps the active tuning as a single scale. Undo restores the previous set. Clearing a set resets its slot assignments, so check any Set position automation before continuing.

Undo and Redo are the curved arrows at the upper right of the grey panel. Inton remembers up to 32 edits, including scale loads, additions, replacements, deletions, reordering and applied scale edits. Undo does not change reference frequency, transpose, Morph or power settings. Browsing and playback automation do not create undo steps.

Ctrl+Z undoes an edit. Ctrl+Shift+Z or Ctrl+Y redoes it. These shortcuts apply when you are not typing in a text field. Apply or Cancel an open draft before using Undo or Redo. Closing the interface keeps the history; loading a saved DAW state clears it.

## Automate tuning changes

Automate Set position to select numbered slots. Its range stays at 1 through 32, even when your set is smaller. Slot numbers are stable when you add or clear scales. Replacing or reordering their contents deliberately changes what those automation positions will play.

Morph sets the transition time between tunings, from 0 ms to ten seconds. It appears in the Scale set toolbar. At 0 ms, tuning changes immediately. During a morph, each MIDI note moves between its old and new pitch, even if the scales have different note counts or periods. Selecting another scale starts from the pitches currently sounding.

A receiving instrument must support continuous MTS retuning to glide held notes. Some instruments apply new tuning only to new notes. Library audition switches immediately and does not use Morph.

Tuning updates run in real time, including when transport is stopped. Record or export automated tuning changes in real time. Faster-than-real-time offline rendering is not supported.

## Read the scale information

The wheel shows one complete repeating span, called the period. The root is at the top. The next repeat meets the same point on the wheel.

Hover over the main wheel to show grey 12 EDO reference marks for one-octave scales. The marks disappear when the pointer leaves. Hover a scale note to read its distance above or below the nearest reference pitch beneath the wheel. The reference shares the scale root; it does not compare MIDI key assignments or judge whether a note is correct. For other periods, hovering a note shows its cents and frequency ratio from the root, without a 12 EDO reference ring.

The orange summary gives the note count and period. For example, 17 equal steps · 2 octaves means seventeen equally spaced notes across two octaves. Hover it to see the frequency ratio and cents. One octave is 1200 cents or a 2:1 frequency ratio. Periods that are not whole octaves use ratios in the summary.

With default keyboard mapping, each consecutive MIDI key advances one scale degree, including black keys. Octave: +7 keys means a note and the key seven MIDI steps above it are an octave apart. If you count the starting key as the first, the repeat is the eighth key. Piano-key names do not describe the pitches of a microtonal scale.

The root marks where the scale pattern begins. The reference key anchors the tuning to a frequency. They need not be the same key. MIDI 69 is normally A4; MIDI 60 is normally middle C. A custom keyboard mapping can choose different keys. Hover the mapping information to see the reference used by that scale.

## Edit a scale

Close the browser and click the cog beside the scale title. The editor changes a copy in your project, leaving the Library file untouched.

The editor shows up to sixteen notes per page. Read down the left column, then down the right. The count includes the root at zero cents. The final repeat point is the Period, not an extra note.

Edit note values in cents above the root. The − and + beside the count remove the last note or add one before the period. The small + beside a note inserts a pitch halfway to the next one. The × removes that note. Inserting or removing a note leaves the other pitch values unchanged. The root and period cannot be deleted.

Period changes the repeat point without moving the other notes. Offsets shows each note's deviation from equal spacing for the current count and period. Switching this display on or off does not change the tuning.

Space equally generates a draft with your chosen note count and period. It replaces the intervals and resets keyboard mapping. For a seven-note octave scale, generate seven notes with a 1200-cent period, then adjust the six notes between the root and the repeat.

Keyboard mapping lets you set the root and reference keys. Imported mappings stay attached when you add or remove notes. Use default mapping maps consecutive keys through the edited scale. The default root is MIDI 60 and the reference is MIDI 69.

Extract a repeating region takes part of a larger tuning table and repeats it with default keyboard mapping. Choose its starting degree and interval count. The chosen starting pitch becomes the new root.

Audition draft sends your edits to connected instruments as you work. Apply saves the draft into its destination slot. An inactive slot stays inactive. Cancel or Escape discards the draft and restores the project tuning.

## Audition without replacing a scale

Library audition is available when adding a scale or working with several scales. It sends the selected tuning to your instruments without replacing a slot. Play the receiving instrument to hear it.

Turning audition off, loading a scale, activating a slot, choosing Show active, or closing the editor returns to the project tuning. Automation continues underneath audition. Temporary auditions and unapplied drafts are not saved with the project.

## Reference, transpose and connections

The tuning fork and Hz value in the footer set the frequency of the scale's reference key, from 400 to 480 Hz. This is usually MIDI 69, but a keyboard mapping can choose another key.

Transpose shifts all pitches by up to 24 chromatic semitones in either direction. Reference and transpose apply during audition and morphing too.

The link icon and client count show registered MTS receivers. The power button enables or disables Inton's tuning master. Turning it off releases the connection and preserves your project settings.

MTS tuning reaches compatible instruments without routing audio or MIDI through Inton. If Inton is in an effect chain, it passes stereo audio and notes through. Only one MTS master can own the shared connection at a time.

If Tuning blocked appears, check whether another tuning master is running. Stop that master if you want Inton to take over. Inton waits for the connection to become available.

After a crash, the connection can retain an old master registration. If the warning persists with no other master running, click it and choose Reset and reconnect. This republishes Inton's current tuning and resets the shared client counter. Cancel or Escape closes the panel without resetting.

After a reset, loaded instruments may still receive tuning while the count is wrong. Disable all receivers, then enable them again, or unload and reload them together. Reset and reconnect is available only when the installed MTS library supports it.

## Add your own files

The + in the Library tools imports a Scala .scl file and its matching .kbm keyboard mapping. You can also copy files into your scale folder and click the circular Rescan icon. A .kbm pairs with a .scl when their names match before the extension. Subfolders become categories.

The Library cog opens folder settings. Choose another absolute path and click Apply folder, or open the folder in your file manager. Escape or the close button dismisses unapplied changes.

Invalid scales show an error and cannot replace a valid tuning. Each scale must resolve all 128 MIDI notes; mappings that leave keys unmapped are rejected. Inton does not follow symlinks in the scale folder.

Inton saves the scale and keyboard-mapping data in the DAW project. The project can recall its tunings even if you later move or delete the original files. Favorites and interface preferences are separate from the project.

## Zoom, appearance and help

Click oiko audio, or right-click anywhere in the header, for interface zoom, the Listening guide and a short Quick guide. The moon or sun switches between dark and light themes.

The folder and list icons open Library and Scale set in the same panel. Click the open tab again, press Escape, or click outside the browser area to close it. Browsing and Playing ideas do not resize the window.

## Installation and further reading

Installation paths, cross-platform build commands and update instructions are in the installation guide.

[Installation and building](docs/installing.md)

[Listening guide and chord exercises](docs/first-scales.md)

[Electronic detunings and exact offsets](docs/electronic-scales.md)

[Testing and host checklist](../../docs/testing.md#inton)

[Licenses and third-party code](THIRD_PARTY.md)
