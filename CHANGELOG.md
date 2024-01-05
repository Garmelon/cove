# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

Procedure when bumping the version number:
1. Update dependencies and flake in a separate commit
2. Set version number in `Cargo.toml`
3. Add new section in this changelog
4. Run `cargo run help-config > CONFIG.md`
5. Commit with message `Bump version to X.Y.Z`
6. Create tag named `vX.Y.Z`
7. Fast-forward branch `latest`
8. Push `master`, `latest` and the new tag

## Unreleased

### Added
- Support for setting window title
- More information to room list heading

### Removed
- Key binding to open present page

## v0.8.0 - 2024-01-04

### Added
- Support for multiple euph server domains
- Support for `TZ` environment variable
- `time_zone` config option
- `--domain` option to `cove export` command
- `--domain` option to `cove clear-cookies` command
- Domain field to "connect to new room" popup
- Welcome info box next to room list

### Changed
- The default euph domain is now https://euphoria.leet.nu/ everywhere
- The config file format was changed to support multiple euph servers with different domains.
  Options previously located at `euph.rooms.*` should be reviewed and moved to `euph.servers."euphoria.leet.nu".rooms.*`.
- Tweaked F1 popup
- Tweaked chat message editor when nick list is foused
- Reduced connection timeout from 30 seconds to 10 seconds

### Fixed
- Room deletion popup accepting any room name
- Duplicated key presses on Windows

## v0.7.1 - 2023-08-31

### Changed
- Updated dependencies

## v0.7.0 - 2023-05-14

### Added
- Auto-generated config documentation
  - in [CONFIG.md](CONFIG.md)
  - via `help-config` CLI command
- `keys.*` config options
- `measure_widths` config option

### Changed
- Overhauled widget system and extracted generic widgets to [toss](https://github.com/Garmelon/toss)
- Overhauled config system to support auto-generating documentation
- Overhauled key binding system to make key bindings configurable
- Redesigned F1 popup. It can now be toggled with F1 like the F12 log
- The F12 log can now be closed with escape
- Some more small UI fixes and adjustments to the new key binding system
- Reduced tearing when redrawing screen
- Split up project into sub-crates
- Simplified flake dependencies

## v0.6.1 - 2023-04-10

### Changed
- Improved JSON export performance
- Always show rooms from config file in room list

### Fixed
- Rooms reconnecting instead of showing error popups

## v0.6.0 - 2023-04-04

### Added
- Emoji support
- `flake.nix`, making cove available as a nix flake
- `json-stream` room export format
- Option to export to stdout via `--out -`
- `--verbose` flag

### Changed
- Non-export info is now printed to stderr instead of stdout
- Recognizes links without scheme (e.g. `euphoria.io` instead of `https://euphoria.io`)
- Rooms waiting for reconnect are no longer sorted to bottom in default sort order

### Fixed
- Mentions not being stopped by `>`

## v0.5.2 - 2023-01-14

### Added
- Key binding to open present page

### Changed
- Always connect to &rl2dev in ephemeral mode
- Reduce amount of messages per &rl2dev log request

## v0.5.1 - 2022-11-27

### Changed
- Increase reconnect delay to one minute
- Print errors that occurred while cove was running more compactly

## v0.5.0 - 2022-09-26

### Added
- Key bindings to navigate nick list
- Room deletion confirmation popup
- Message inspection popup
- Session inspection popup
- Error popup when external editor fails
- `rooms_sort_order` config option

### Changed
- Use nick changes to detect sessions for nick list
- Support Unicode 15

### Fixed
- Cursor being visible through popups
- Cursor in lists when highlighted item moves off-screen
- User disappearing from nick list when only one of their sessions disconnects

## v0.4.0 - 2022-09-01

### Added
- Config file and `--config` cli option
- `data_dir` config option
- `ephemeral` config option
- `offline` config option and `--offline` cli flag
- `euph.rooms.<name>.autojoin` config option
- `euph.rooms.<name>.username` config option
- `euph.rooms.<name>.force_username` config option
- `euph.rooms.<name>.password` config option
- Key binding to change rooms sort order
- Key bindings to connect to/disconnect from all rooms
- Key bindings to connect to autojoin rooms/disconnect from non-autojoin rooms
- Key bindings to move to parent/root message
- Key bindings to view and open links in a message

### Changed
- Some key bindings in the rooms list

### Fixed
- Rooms being stuck in "Connecting" state

## v0.3.0 - 2022-08-22

### Added
- Account login and logout
- Authentication dialog for password-protected rooms
- Error popups in rooms when something goes wrong
- `--ephemeral` flag that prevents cove from storing data permanently
- Key binding to download more logs

### Changed
- Reduced amount of unnecessary redraws
- Description of `export` CLI command

### Fixed
- Crash when connecting to nonexistent rooms
- Crash when connecting to rooms that require authentication
- Pasting multi-line strings into the editor

## v0.2.1 - 2022-08-11

### Added
- Support for modifiers on special keys via the [kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)

### Fixed
- Joining new rooms no longer crashes cove
- Scrolling when exiting message editor

## v0.2.0 - 2022-08-10

### Added
- New messages are now marked as unseen
- Sub-trees can now be folded
- Support for pasting text into editors
- More readline-esque editor key bindings
- Key bindings to move to prev/next sibling
- Key binding to center cursor on screen
- More scrolling key bindings
- JSON message export
- Export output path templating
- Support for exporting multiple/all rooms at once

### Changed
- Reorganized export command
- Slowed down room history download speed

### Fixed
- Chat rendering when deleting and re-joining a room
- Spacing in some popups

## v0.1.0 - 2022-08-06

Initial release
