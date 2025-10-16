# Timeline Filter

> A personalized AT Protocol feed generator with per-user timeline filtering and repost control.

**Timeline Filter** is a fork of [Supercell](https://github.com/astrenoxcoop/supercell) that extends the feed generator capabilities to support personalized timeline filtering. Instead of consuming from Jetstream, it polls `app.bsky.feed.getTimeline` to provide users with filtered versions of their Following feed, with granular control over reposts.

## Key Features

- **Per-User Timeline Filtering**: Each user can configure their own filters
- **Repost Control**: Block reposts from specific users while keeping their original posts
- **Timeline Polling**: Uses `getTimeline()` API instead of Jetstream for access to hydrated data
- **OAuth Token Management**: Secure per-user authentication
- **Based on Supercell**: Inherits Supercell's robust architecture, caching, and HTTP handling

## Changes from Supercell

This fork modifies the core consumer architecture:

- ✨ **New**: Timeline consumer that polls `app.bsky.feed.getTimeline` per user
- ✨ **New**: Per-user configuration with blocked reposter lists
- ✨ **New**: OAuth token storage and management
- 🔄 **Modified**: Consumer architecture to support user-based polling instead of Jetstream
- 🔄 **Modified**: Configuration format to include user-specific settings

## Configuration

The following environment variables are used:

* `HTTP_PORT` - The port to listen on for HTTP requests.
* `EXTERNAL_BASE` - The hostname of the feed generator.
* `DATABASE_URL` - The URL of the database to use.
* `POLL_INTERVAL` - How often to poll timelines (default: `30s`)
* `VMC_TASK_ENABLE` - Whether or not to enable the VMC (verification method cache) tasks. Default `true`.
* `CACHE_TASK_ENABLE` - Whether or not to enable the cache tasks. Default `true`.
* `CACHE_TASK_INTERVAL` - The interval to run the cache tasks. Default `3m`.
* `CLEANUP_TASK_ENABLE` - Whether or not to enable the cleanup tasks. Default `true`.
* `CLEANUP_TASK_INTERVAL` - The interval to run the cleanup tasks. Default `1h`.
* `CLEANUP_TASK_MAX_AGE` - The maximum age of a post before it is considered stale and deleted from storage. Default `48h`.
* `PLC_HOSTNAME` - The hostname of the PLC server to use for VMC tasks. Default `plc.directory`.
* `TIMELINE_FEEDS` - The path to the timeline feeds configuration file.
* `RUST_LOG` - Logging configuration. Defaults to `timeline_filter=debug,info`

### Timeline Feed Configuration

The timeline feed configuration file is a YAML file that contains per-user feed settings:

```yaml
timeline_feeds:
  - did: "did:plc:user123abc"
    feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/filtered-timeline"
    name: "My Filtered Timeline"
    description: "Following feed without annoying reposts"
    oauth_token: "your-oauth-token-here"
    blocked_reposters:
      - "did:plc:annoying-user1"
      - "did:plc:annoying-user2"
    poll_interval: "30s"  # Optional: override global interval
```

Each user can configure:
- Their DID
- The feed URI to publish under
- A list of DIDs whose reposts should be filtered out
- An OAuth token for authenticated timeline access
- Optional custom polling interval

## How It Works

1. **Authentication**: Users authenticate via OAuth and provide their token
2. **Polling**: The consumer periodically calls `getTimeline()` for each configured user
3. **Filtering**: Posts are filtered based on the user's blocked reposter list
4. **Indexing**: Filtered posts are stored in the database per user
5. **Serving**: The feed generator serves the filtered timeline via the standard feed API

## Installation

```bash
# Clone the repository
git clone https://github.com/YOUR-USERNAME/timeline-filter
cd timeline-filter

# Set up environment variables
cp .env.example .env
# Edit .env with your configuration

# Set up the database
sqlx database create
sqlx migrate run

# Build and run
cargo build --release
./target/release/timeline-filter
```

## Attribution

This project is based on [Supercell](https://github.com/astrenoxcoop/supercell) by [The Astrenox Cooperative](https://astrenox.coop/), licensed under the MIT License.

We are grateful for their excellent work on the original feed generator architecture, which made this project possible.

### Original Supercell Features

Supercell's core features that Timeline Filter inherits:

- Lightweight and configurable architecture
- Built-in caching system
- Verification method cache (VMC) for JWT validation
- Cleanup tasks for stale data
- Robust HTTP server with CORS support
- SQLite database with migrations

For more information about Supercell, visit the [original repository](https://github.com/astrenoxcoop/supercell).

## Development Status

⚠️ **Alpha**: This project is in early development. Core functionality is being implemented. Use at your own risk.

## License

This project is open source under the MIT license.

Copyright (c) 2025 Lucas

Portions derived from Supercell:
Copyright (c) 2024 The Astrenox Cooperative

See [LICENSE](LICENSE) for the full license text.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## Links

- [Original Supercell Repository](https://github.com/astrenoxcoop/supercell)
- [The Astrenox Cooperative](https://astrenox.coop/)
- [AT Protocol Documentation](https://atproto.com)
- [Bluesky](https://bsky.app)
