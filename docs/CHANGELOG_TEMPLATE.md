# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- 

### Changed
- 

### Deprecated
- 

### Removed
- 

### Fixed
- 

### Security
- 

## [0.7.0] - 2024-01-01

### Added
- Initial release of Grunner
- Fuzzy application search with fuzzy-matcher
- Colon command system for extended functionality
- File search integration with plocate (`:f` command)
- Content search integration with ripgrep (`:fg` command)
- GNOME Shell search provider integration (`:s` command)
- Obsidian vault integration (`:ob` and `:obg` commands)
- Power management controls (suspend, restart, power off, logout)
- Configuration system with TOML file support
- Automatic theme adaptation (light/dark mode, accent color)
- Application icon and description display
- Terminal application support with auto-detection
- Custom command definition via configuration
- Application caching for faster startup
- Settings button for quick config editing
- Keyboard navigation (arrows, page up/down, escape)

### Technical
- Built with Rust 2024 edition
- GTK4 and libadwaita 1.6+ for modern UI
- Async operations with tokio runtime
- D-Bus integration with zbus
- Binary serialization with bincode for caching
- CSS styling embedded in binary
- XDG standards compliance for config and cache
- Systemd integration for power management

## [0.6.0] - YYYY-MM-DD

### Added
- 

### Changed
- 

### Fixed
- 

## [0.5.0] - YYYY-MM-DD

### Added
- 

### Changed
- 

### Fixed
- 

## [0.4.0] - YYYY-MM-DD

### Added
- 

### Changed
- 

### Fixed
- 

## [0.3.0] - YYYY-MM-DD

### Added
- 

### Changed
- 

### Fixed
- 

## [0.2.0] - YYYY-MM-DD

### Added
- 

### Changed
- 

### Fixed
- 

## [0.1.0] - YYYY-MM-DD

### Added
- 

### Changed
- 

### Fixed
- 

---

## Versioning Guidelines

### Semantic Versioning
- **MAJOR** version (X.0.0): Incompatible API changes
- **MINOR** version (0.X.0): New functionality (backward compatible)
- **PATCH** version (0.0.X): Bug fixes (backward compatible)

### Changelog Entry Types
- **Added**: New features
- **Changed**: Changes in existing functionality
- **Deprecated**: Soon-to-be removed features
- **Removed**: Removed features
- **Fixed**: Bug fixes
- **Security**: Vulnerability fixes

## Release Process

1. Update version in `Cargo.toml`
2. Update this CHANGELOG.md with new version section
3. Commit changes with message "chore: release vX.Y.Z"
4. Create git tag: `git tag vX.Y.Z`
5. Push tag: `git push origin vX.Y.Z`
6. Create GitHub release with changelog entries

## Links

[Unreleased]: https://github.com/Nihmar/grunner/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/Nihmar/grunner/releases/tag/v0.7.0
[0.6.0]: https://github.com/Nihmar/grunner/compare/v0.6.0...v0.7.0
[0.5.0]: https://github.com/Nihmar/grunner/compare/v0.5.0...v0.6.0
[0.4.0]: https://github.com/Nihmar/grunner/compare/v0.4.0...v0.5.0
[0.3.0]: https://github.com/Nihmar/grunner/compare/v0.3.0...v0.4.0
[0.2.0]: https://github.com/Nihmar/grunner/compare/v0.2.0...v0.3.0
[0.1.0]: https://github.com/Nihmar/grunner/releases/tag/v0.1.0