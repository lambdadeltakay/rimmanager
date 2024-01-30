# RimManager

[![Rust](https://github.com/lambdadeltakay/rimmanager/actions/workflows/master_release.yml/badge.svg)](https://github.com/lambdadeltakay/rimmanager/actions/workflows/master_release.yml)

An open-source rust RimWorld mod manager

Inspired by the wonderful [RimSort](https://github.com/RimSort/RimSort) application

Please note this program is not finished nor tested extensively and may break at any time for any reason

Please review this entire readme before using this application

## Features and their implementation status

- [x] Parsing RimWorld basic XML files needed for this program
- [x] Basic EGUI-based user interface
- [x] Mod list validation and writing support
- [ ] Good EGUI-based user interface
- [ ] Auto mod list fixing
- [ ] Optimization and organization
- [ ] Localization
- [ ] Download and update mods through the SteamWorks API without the official Steam library, for those who acquired RimWorld outside of Steam.

## External dependencies

- Linux (X11)
  - Debian and derivatives: `apt install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev`
  - Arch and derivatives: `pacman -S libxcb libxkbcommon openssl`
  - Gentoo: `emerge --ask x11-libs/libxcb x11-libs/libxkbcommon dev-libs/openssl`
- Linux (Wayland)
  - TODO
- Windows
  - TODO
- MacOS
  - TODO

## Known issues (PLEASE READ)

- The program assumes you have opened RimWorld at least once. Please open RimWorld at least once before opening this program.
- In no way does this program try to watch changes to mod directories. Anything that happens between calls to scan installation will not be caught and may result in catastrophic failure
- The resulting ModConfig.xml saved is not beautified
- Mods that do not support your installed version of RimWorld will not be visible
- I'm not good at UI so all the UI is weirdness right now. It probably won't work on a low-resolution screen
- The default font EGUI uses cannot render non latin fonts. Later I will make it load a font from your system.
- The mod manager does not handle circular dependencies well
