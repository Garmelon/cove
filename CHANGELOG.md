# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

Procedure when bumping the version number:
1. Update dependencies in a separate commit
2. Set version number in `Cargo.toml`
3. Add new section in this changelog
4. Commit with message `Bump version to vX.Y.Z`
5. Create tag named `vX.Y.Z`
6. Fast-forward branch `latest`
7. Push `master`, `latest` and the new tag

## Unreleased

### Added
- New messages are now marked as unseen
- Sub-trees can now be folded
- More readline-esque editor key bindings
- Key bindings to move to prev/next sibling
- Key binding to center cursor on screen

### Changed
- Slowed down room history download speed

### Fixed
- Chat rendering when deleting and re-joining a room
- Spacing in some popups

## v0.1.0 - 2022-08-06

Initial release
