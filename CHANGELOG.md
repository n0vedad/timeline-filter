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

[0.2.0]: https://github.com/astrenoxcoop/supercell/compare/0.1.2..0.2.0
[0.1.2]: https://github.com/astrenoxcoop/supercell/compare/0.1.1..0.1.2
[0.1.1]: https://github.com/astrenoxcoop/supercell/compare/0.1.0..0.1.1

