# Oiko Inton

[Product page](https://oikoaudio.com/inton/)

> **Beta 0.5.0-beta.1.** Sound, controls and automation mappings may change between beta releases. Read the [release notes](RELEASE_NOTES.md) before updating existing projects.

**[Download Inton](https://oikoaudio.com/downloads/#inton)** for macOS, Windows, and Linux.

Oiko Inton sends MTS-ESP tuning to every compatible instrument in your DAW project. Choose one scale for the whole track, or build a Scale set and automate changes between its scales. Your DAW project saves the scales inside Inton's state.

Inton is a CLAP and VST3 plugin for Linux, Windows and macOS.

## Start playing

1. Insert Inton in your DAW. It starts with 12 EDO, the standard tuning of twelve equally spaced notes per octave.
2. Open an instrument that supports MTS-ESP. If the instrument has an MTS tuning option, turn it on. Play notes on that instrument, because Inton makes no sound itself.
3. Click the folder icon to open the Library. With one scale loaded, clicking a scale in the list changes the tuning immediately.
4. Click the folder icon again, or press Escape, to close the Library.

Chords on a sawtooth patch make tuning differences easy to hear. Low sine tones hide small tuning changes. Switch back to 12 EDO to compare.

Drag the bottom-right corner to choose a zoom level from 50% to 200% in 25% steps. The selected percentage appears in the center. Release the mouse button to resize the window. The aspect ratio stays fixed. Press Escape during the drag to cancel.

## Find a scale

Opening the Library selects the current scale and puts the cursor in the search field. Type to search names, categories, tags and descriptions. If an existing filter hides the current scale, opening the Library clears that filter.

The star shows only Favorites. The factory icon shows only factory scales. The person icon shows only your own files. Click the selected filter again to show all sources. Click a star beside a scale to change your Favorites.

The funnel opens the category filter. The Listening guide category holds eight scales with listening exercises. Changing your Favorites does not remove scales from this category. When a scale has guidance, a Playing ideas button appears below the scale information in both the main view and the browser. Click it to read the suggestion.

The arrows beside the tabs step through the filtered Library. Up and Down on the keyboard move from search into the list and continue from the selected scale. With one scale loaded, each step changes the current tuning. While you add to or arrange a Scale set, each step only selects a scale so you can inspect it.

The factory library contains 49 tunings, including electronic detunings, equal divisions and Carlos Alpha, Beta and Gamma. Your own scale files appear in the same list.

## Use several scales

The list icon opens Scale set. A Scale set holds up to 32 numbered slots, each with one scale, so you can prepare tuning changes for a track.

Drag a Library scale onto the Scale set tab to add it to the end of the set. The tab flashes when it accepts the scale. Inton does not activate the new scale or switch tabs. Open Scale set to see it.

Click the dashed + row to browse for another scale. In this mode, select a Library scale and click Add, double-click it, or press Right to add it. Click the speaker icon to hear the selected scale before you add it. The icon is orange while audition is on.

Click a slot's dot or double-click its row to activate it. The orange dot marks the active slot. A single click on the row selects it for inspection without activating it. The arrows beside the tabs select the previous or next occupied slot, skipping gaps. The number to their right is Set position, the parameter your DAW can automate.

To replace an occupied slot, click its circular-arrows button, then choose a Library scale and load it. Escape cancels replacement. To fill an empty slot, select it before loading a scale. You can also drag a Library scale onto any slot to replace its contents.

Drag a populated slot to reorder the scales. A line shows where the slot will land. If you move the active scale, it stays active in its new slot.

## Clear slots and undo changes

The × on a slot clears its scale. If occupied slots follow it, the empty slot stays, so the later slots keep their automation numbers. If no occupied slots follow it, Inton removes that row. Adding a scale with + always appends a new slot and never fills a gap.

Deleting the active scale activates the first remaining scale. If no scales remain, Inton keeps sending the last tuning. If automation selects an empty or unused position, Inton also keeps the current tuning and adds no rows.

The trash button at the bottom of Scale set clears the set and keeps the active tuning as a single scale. Undo restores the previous set. Clearing a set empties every slot, so check any Set position automation afterwards.

Undo and Redo are the curved arrows at the upper right of the grey panel. Inton remembers up to 32 edits. Scale loads, additions, replacements, deletions, reordering and applied scale edits each count as one edit. Undo does not change reference frequency, transpose, Morph or power settings. Browsing and playback automation do not create undo steps.

Ctrl+Z undoes an edit. Ctrl+Shift+Z or Ctrl+Y redoes it. These shortcuts apply when you are not typing in a text field. Apply or Cancel an open draft before using Undo or Redo. Closing the interface keeps the history. Loading a saved DAW state clears it.

## Automate tuning changes

Automate Set position to select numbered slots. Its range is always 1 to 32, even when your set is smaller. Adding or clearing scales does not renumber the other slots. Replacing or reordering slots changes which scale each automation position plays.

Morph in the Scale set toolbar sets the transition time between tunings, from 0 ms to ten seconds. At 0 ms, tuning changes immediately. During a morph, each MIDI note moves between its old and new pitch, even if the scales have different note counts or periods. If you select another scale during a morph, the new morph starts from the current in-between pitches.

Held notes glide only in instruments that support continuous MTS retuning. Some instruments apply new tuning only to new notes. Library audition switches immediately and does not use Morph.

Inton updates tuning in real time, including when transport is stopped. Morph timing follows the computer's clock, not the DAW's playback position, so record or export automated tuning changes in real time. Inton does not support faster-than-real-time offline rendering.

## Read the scale information

The wheel shows one period, the span after which the scale repeats. The root is at the top. The first note of the next repeat lands on the same point.

Hover over the main wheel to show grey 12 EDO reference marks for one-octave scales. The marks disappear when the pointer leaves. Hover a scale note to see, beneath the wheel, the nearest 12 EDO interval and how many cents the note lies above or below it. The 12 EDO marks start from the scale root. They do not compare MIDI key assignments or judge whether a note is correct. For other periods, hovering a note shows its cents and frequency ratio from the root, without a 12 EDO reference ring.

The orange summary gives the note count and period. For example, 17 equal steps · 2 octaves means seventeen equally spaced notes across two octaves. Hover it to see the frequency ratio and cents. One octave is 1200 cents or a 2:1 frequency ratio. Periods that are not whole octaves use ratios in the summary.

With default keyboard mapping, each consecutive MIDI key advances one scale degree, including black keys. Octave: +7 keys means a note and the key seven MIDI steps above it are an octave apart. Counting the starting key as the first, the repeat falls on the eighth key. Piano-key names do not describe the pitches of a microtonal scale.

The root is the key where the scale pattern begins. The reference key is the key tuned to the reference frequency. They can be different keys. MIDI 69 is normally A4, and MIDI 60 is normally middle C. A custom keyboard mapping can choose different keys. Hover the mapping information to see the reference used by that scale.

## Edit a scale

Close the browser and click the cog beside the scale title. The editor changes a copy stored in your project and never writes to the Library file.

The editor shows up to 24 notes per page, in three columns of eight. Read down each column, from left to right. The note count includes the root at 0 cents. The Period field sets the repeat point, which is not an extra note.

Edit note values in cents above the root. The − and + beside the note count remove the last note or add one before the period. The small + beside a note inserts a pitch halfway to the next one. The × beside a note removes it. Inserting or removing a note leaves the other pitch values unchanged. You cannot delete the root or the period.

Changing Period moves the repeat point without moving the other notes. Offsets (Δ) shows each note's deviation from equal spacing for the current note count and period. Switching this display on or off does not change the tuning.

Equal scale… creates an equal division. Set the Period first, then open Equal scale…, choose the number of Notes and click Replace. Inton spaces that many notes evenly across the period. This replaces every interval and resets the keyboard mapping. For a seven-note octave scale, use a 1200-cent Period and seven Notes, then adjust the six notes between the root and the repeat. For seven notes over two octaves (7ED4), use a 2400-cent Period.

Keyboard mapping sets the root and reference keys. An imported mapping stays attached when you add or remove notes. Use default mapping discards the imported mapping and maps consecutive keys through the edited scale. The default root is MIDI 60 and the default reference is MIDI 69.

Audition draft sends your edits to connected instruments as you work. Apply saves the draft into its destination slot. If that slot is inactive, it stays inactive. Cancel or Escape discards the draft and restores the project tuning.

## Audition without replacing a scale

Library audition is available while you add a scale or work with a Scale set. It sends the selected tuning to your instruments without replacing a slot. Play the receiving instrument to hear it.

Inton returns to the project tuning when you turn audition off, load a scale, activate a slot, choose Show active or close the editor. Automation keeps changing the project tuning during audition. The project does not save auditions or unapplied drafts.

## Reference, transpose and connections

The tuning fork and Hz value in the footer set the frequency of the scale's reference key, from 400 to 480 Hz. The reference key is usually MIDI 69, but a keyboard mapping can choose another key.

Transpose shifts all pitches by up to 24 chromatic semitones up or down. Reference and transpose also apply during audition and morphing.

The link icon and client count show how many MTS receivers are registered. The power button turns Inton's tuning master on or off. Turning it off releases the MTS-ESP connection and keeps your project settings.

Compatible instruments receive MTS tuning without routing audio or MIDI through Inton. If Inton is in an effect chain, it passes stereo audio and notes through unchanged. Only one MTS master can own the shared connection at a time.

If Tuning blocked appears, another tuning master may be running. Stop that master if you want Inton to take over. Inton takes the connection as soon as it becomes free.

After a crash, the connection can keep an old master registration. If the warning persists with no other master running, click it and choose Reset and reconnect. Inton then sends its current tuning again and resets the shared client counter. Cancel or Escape closes the panel without resetting.

After a reset, loaded instruments may still receive tuning while the client count is wrong. To correct the count, turn off MTS in all receivers and turn it on again, or unload and reload them together. Reset and reconnect appears only when the installed MTS library supports it.

## Add your own files

The + in the Library tools imports a Scala .scl file and its matching .kbm keyboard mapping. You can also copy files into your scale folder and click the circular Rescan icon. Inton pairs a .kbm file with the .scl file of the same name. Subfolders become categories.

The Library cog opens folder settings. Enter another absolute path and click Apply folder, or open the folder in your file manager. Escape or the close button discards unapplied changes.

An invalid scale shows an error and cannot replace a valid tuning. Each scale must give a pitch to all 128 MIDI notes. Inton rejects keyboard mappings that leave keys unmapped. Inton does not follow symlinks in the scale folder.

Inton saves the scale and keyboard-mapping data in the DAW project. The project recalls its tunings even if you later move or delete the original files. Inton stores Favorites and interface preferences outside the project.

## Zoom, appearance and help

Click oiko audio, or right-click anywhere in the header, to open a menu with interface zoom, the Listening guide and a short Quick guide. The moon or sun switches between dark and light themes.

The folder and list icons open Library and Scale set in the same panel. To close the panel, click the open tab again, press Escape, or click outside the browser area. Browsing and Playing ideas do not resize the window.

## Installation and further reading

The installation guide lists installation paths, build commands for each platform and update steps.

[Installation and building](docs/installing.md)

[Listening guide and chord exercises](docs/first-scales.md)

[Electronic detunings and exact offsets](docs/electronic-scales.md)

[Testing and host checklist](../../docs/testing.md#inton)

[Licenses and third-party code](THIRD_PARTY.md)

## Build from source

This product is part of the [Oiko Audio workspace](../../README.md). Shared DSP, typography, window handling and upstream patches are at the repository root. Clone the complete workspace and use the common `cargo xtask` bundler. See the [engineering principles](../../docs/engineering-principles.md) and [patch register](../../docs/upstream-patches.md).
