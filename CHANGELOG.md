# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

Procedure when bumping the version number:
1. Update dependencies in a separate commit
2. Set version number in `Cargo.toml`
3. Add new section in this changelog
4. Commit with message `Bump version to X.Y.Z`
5. Create tag named `vX.Y.Z`
6. Fast-forward branch `latest`
7. Push `master`, `latest` and the new tag

## Unreleased

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
