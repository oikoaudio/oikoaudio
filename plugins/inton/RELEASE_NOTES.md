# Oiko Inton 0.5.0-beta.2

This beta fixes the editor size on scaled Linux desktops. Sound, controls and automation mappings may change between beta releases. Before updating, keep the previous plugin version and back up existing projects.

## Changes

- On Linux, the editor now opens at the right size on desktops that scale X11 apps, such as KDE Plasma on Wayland at 200% with legacy apps scaling themselves. Before, the host window kept the unscaled size and cropped the enlarged editor. With VST3, the window can open at the smaller size and then grow to fit the editor.

## Compatibility

Parameter identities, Scale set slot numbering and saved projects are unchanged from 0.5.0-beta.1.

## Formats and known limitations

Inton ships as CLAP and VST3 for Linux x86-64, Windows x86-64 and universal macOS. Builds are unsigned, and macOS builds are not notarized. Audio Unit stays disabled until the editor problem in Logic is fixed.

[Manual and installation](https://oikoaudio.com/inton/)
