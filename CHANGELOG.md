## [0.5.1] - 2024-11-15

### Changed

- Denylist support and admin page
- Setting version to 0.5.1

## [0.5.0] - 2024-11-15

### Changed

- Atmosphere dev feed matcher updates
- Updating atmosphere dev feed
- Added rhai function to compare dates with duration strings
- Adding duration based date filter to atmosphere dev feed
- Setting version to 0.5.0
- 0.5.0

## [0.4.2] - 2024-11-13

### Changed

- Setting version to 0.4.2
- 0.4.2

### Fixed

- Fixed min/max typo that floored score to 0

## [0.4.1] - 2024-11-12

### Added

- Added app.bsky.feed.like to build_aturi

### Changed

- Updating atmosphere dev feed matcher
- Adding support for local test data
- Adding bail mechanics to atmostphere_dev feed
- Cleanup task as part of data retention policy
- Updating atmosphere_dev feed matcher to calc score
- Setting version to 0.4.1
- 0.4.1

## [0.4.0] - 2024-11-12

### Added

- Adding sequence matcher helper

### Changed

- Cleaned up and refactored matchers
- Score can be incremented
- Feed caching
- Added dropsonde to test rhai scripts
- Setting version to 0.4.0
- 0.4.0

### Fixed

- Adding test for rhai link matching

## [0.3.1] - 2024-11-09

### Changed

- Cleaning up changelog
- Adding value context to at-uri compose errors
- Setting version to 0.3.1
- 0.3.1

## [0.3.0] - 2024-11-09

### Added

- Support per-matcher AT-URI

### Changed

- Added org.opencontainers labels
- Downgrading error to info
- Experimental rhai scripting support
- Updating example docker-compose configuration file
- Setting version to 0.3.0
- 0.3.0
- Typo in cargo config
- 0.3.0
- 0.3.0

### Fixed

- Fixing bad build stuff

## [0.2.0] - 2024-11-07

### Changed

- Add describeFeedGenerator endpoint (#1)
- Typo in app.bsky.feed.generator example (#2)
- Attempting to avoid 'Destination buffer is too small' error by creating decoding buffer 3x larger than max size
- Compression configuration option
- Post at-uri matching
- Added match likes playbook
- Updating readme to include COLLECTIONS
- Setting version to 0.2.0
- 0.2.0

## [0.1.2] - 2024-11-05

### Added

- Support did:web
- Added script and documentation for publishing feed records

### Changed

- Cache did verification methods with plc lookups
- Bypass authentication when allow list is empty
- Using i64 values from peer feedback.
- Optionally display deny post
- Setting version to 0.1.2
- 0.1.2

## [0.1.1] - 2024-11-05

### Changed

- Supercell is a configurable feed generator
- Added changelog and git-cliff configuration
- Added bare release playbook
- Updating create-release script to verify required tools exist and support hooks.
- Setting version to 0.1.1
- 0.1.1

[0.5.1]: https://github.com/astrenoxcoop/supercell/compare/0.5.0..0.5.1
[0.5.0]: https://github.com/astrenoxcoop/supercell/compare/0.4.2..0.5.0
[0.4.2]: https://github.com/astrenoxcoop/supercell/compare/0.4.1..0.4.2
[0.4.1]: https://github.com/astrenoxcoop/supercell/compare/0.4.0..0.4.1
[0.4.0]: https://github.com/astrenoxcoop/supercell/compare/0.3.1..0.4.0
[0.3.1]: https://github.com/astrenoxcoop/supercell/compare/0.3.0..0.3.1
[0.3.0]: https://github.com/astrenoxcoop/supercell/compare/0.2.0..0.3.0
[0.2.0]: https://github.com/astrenoxcoop/supercell/compare/0.1.2..0.2.0
[0.1.2]: https://github.com/astrenoxcoop/supercell/compare/0.1.1..0.1.2
[0.1.1]: https://github.com/astrenoxcoop/supercell/compare/0.1.0..0.1.1

