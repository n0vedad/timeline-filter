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
  - **Note**: This is time-based, not count-based! Posts older than this duration are deleted.
  - Example: `48h` keeps ~500-1000 posts, `7d` keeps ~3500-7000 posts, `30d` keeps ~15000-30000 posts.
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

1. **Configuration**: Define users and their filter rules in `config.yml`
2. **Startup**: On startup, the configuration is synchronized to the database
3. **Polling**: The consumer periodically calls `getTimeline()` for each configured user
4. **Filtering**: Posts are filtered based on the user's blocked reposter list
   - When a post has a `reason` field with type `reasonRepost`, the reposter's DID is checked
   - If the reposter DID is in the user's `blocked_reposters` list, the post is filtered out
   - Original posts from blocked users still appear (only their reposts are filtered)
5. **Indexing**: Filtered posts are stored in the database per user's feed URI
6. **Serving**: The feed generator serves the filtered timeline via the standard AT Protocol feed API

## Prerequisites

- Rust (1.70+)
- SQLite 3
- AT Protocol account(s) with OAuth tokens

## Installation

### 1. Clone and Build

```bash
# Clone the repository
git clone https://github.com/YOUR-USERNAME/timeline-filter
cd timeline-filter

# Build the project
cargo build --release
```

### 2. Set Up Database

```bash
# The database will be created automatically on first run
# Migrations are embedded and run automatically via sqlx
```

### 3. Configure Environment

```bash
# Copy the example environment file
cp .env.example .env

# Edit .env with your configuration
nano .env
```

**Required settings in `.env`:**
```bash
HTTP_PORT=4050
EXTERNAL_BASE=https://your-feed-generator.com
DATABASE_URL=sqlite://timeline-filter.db
TIMELINE_FEEDS=config.yml
```

### 4. Configure Timeline Feeds

```bash
# Copy the example timeline feeds configuration
cp config.example.yml config.yml

# Edit config.yml with your users and filters
nano config.yml
```

**Example `config.yml`:**
```yaml
timeline_feeds:
  - did: "did:plc:youruser123"
    feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/youruser-filtered"
    name: "My Filtered Timeline"
    description: "Following feed without annoying reposts"

    oauth:
      access_token: "your-oauth-access-token"
      pds_url: "https://bsky.social"

    filters:
      blocked_reposters:
        - "did:plc:annoying-user-1"
        - "did:plc:spam-account"
```

### 5. Run

**Development Mode:**
```bash
# Use the development script (automatically sets up environment)
./dev-server.sh
```

**Production Mode:**
```bash
# Build and run the release binary
cargo build --release
./target/release/timeline-filter

# Or with custom logging
RUST_LOG=timeline_filter=debug ./target/release/timeline-filter
```

## Getting OAuth Tokens

To use Timeline Filter, you need OAuth access tokens for each user. Here are a few options:

### Option 1: Using an OAuth Flow (Recommended)

Build a simple OAuth application that:
1. Redirects users to AT Protocol OAuth endpoint
2. Exchanges authorization code for access token
3. Saves tokens to `config.yml`

See [AT Protocol OAuth Documentation](https://atproto.com/specs/oauth) for details.

### Option 2: Using App Passwords (Simple but Less Secure)

1. Go to your Bluesky settings → App Passwords
2. Create a new app password
3. Use it to authenticate and get a session token:

```bash
curl -X POST https://bsky.social/xrpc/com.atproto.server.createSession \
  -H "Content-Type: application/json" \
  -d '{
    "identifier": "your-handle.bsky.social",
    "password": "your-app-password"
  }'
```

4. Copy the `accessJwt` from the response
5. Add it to your `config.yml` as the `access_token`

**Note**: App password tokens expire, so you'll need to refresh them periodically.

## Usage

### Starting the Feed Generator

**Development:**
```bash
# Use the development script (includes auto-reload on code changes)
./dev-server.sh
```

**Production:**
```bash
# Start with default settings from .env
./target/release/timeline-filter

# Or with custom environment variables
HTTP_PORT=8080 POLL_INTERVAL=60s ./target/release/timeline-filter
```

### Monitoring

The feed generator provides detailed logging:

```bash
# Info level (default)
RUST_LOG=info ./target/release/timeline-filter

# Debug level (recommended for development)
RUST_LOG=timeline_filter=debug,info ./target/release/timeline-filter

# Trace level (very verbose)
RUST_LOG=timeline_filter=trace ./target/release/timeline-filter
```

**Log output example:**
```
INFO  timeline_filter Starting timeline consumer task feed_count=2
DEBUG Polling timeline user_did="did:plc:user123"
INFO  Processed timeline posts total=50 filtered=48 blocked=2
DEBUG Successfully completed poll indexed=48
```

### Accessing Your Filtered Feed

Once running, your filtered feed is available at:

```
https://your-feed-generator.com/xrpc/app.bsky.feed.getFeedSkeleton?feed=at://did:plc:feedgen/app.bsky.feed.generator/youruser-filtered
```

### Adding to Bluesky

1. Open Bluesky app
2. Go to Feeds → Add Feed
3. Enter your feed URI: `at://did:plc:feedgen/app.bsky.feed.generator/youruser-filtered`
4. The feed will appear in your feed list

## Configuration Reference

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `HTTP_PORT` | No | `4050` | HTTP server port |
| `EXTERNAL_BASE` | Yes | - | Public URL of your feed generator |
| `DATABASE_URL` | No | `sqlite://timeline-filter.db` | SQLite database path |
| `TIMELINE_FEEDS` | Yes | - | Path to timeline feeds YAML config |
| `TIMELINE_CONSUMER_ENABLE` | No | `true` | Enable/disable timeline consumer |
| `POLL_INTERVAL` | No | `30s` | Global default poll interval |
| `CACHE_TASK_ENABLE` | No | `true` | Enable feed caching |
| `CACHE_TASK_INTERVAL` | No | `3m` | Cache refresh interval |
| `CLEANUP_TASK_ENABLE` | No | `true` | Enable cleanup of old posts |
| `CLEANUP_TASK_INTERVAL` | No | `1h` | Cleanup interval |
| `CLEANUP_TASK_MAX_AGE` | No | `48h` | Maximum age of posts to keep |
| `RUST_LOG` | No | `info` | Logging level |

### Timeline Feed Configuration

Each timeline feed in `config.yml` supports:

| Field | Required | Description |
|-------|----------|-------------|
| `did` | Yes | User's DID (must start with `did:`) |
| `feed_uri` | Yes | Feed URI (must start with `at://`) |
| `name` | Yes | Display name for the feed |
| `description` | Yes | Feed description |
| `oauth.access_token` | Yes | OAuth access token |
| `oauth.refresh_token` | No | OAuth refresh token |
| `oauth.expires_at` | No | Token expiration (ISO 8601) |
| `oauth.pds_url` | Yes | PDS URL (e.g., `https://bsky.social`) |
| `filters.blocked_reposters` | No | List of DIDs whose reposts to filter |
| `poll_interval` | No | Custom poll interval (overrides global) |
| `max_posts_per_poll` | No | Max posts per poll (default: 50, max: 100) |

## Advanced Usage

### Multiple Users

You can configure multiple users in the same `config.yml`:

```yaml
timeline_feeds:
  - did: "did:plc:user1"
    feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/user1-filtered"
    # ... user 1 config

  - did: "did:plc:user2"
    feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/user2-filtered"
    # ... user 2 config
```

Each user gets their own filtered feed with independent filter rules.

### Custom Poll Intervals

You can set different poll intervals for different users:

```yaml
timeline_feeds:
  # Power user - poll every 10 seconds
  - did: "did:plc:poweruser"
    poll_interval: "10s"
    max_posts_per_poll: 100
    # ...

  # Casual user - poll every 5 minutes
  - did: "did:plc:casualuser"
    poll_interval: "5m"
    max_posts_per_poll: 50
    # ...
```

## Troubleshooting

### "Timeline consumer enabled but no timeline feeds configured"

**Solution**: Make sure `TIMELINE_FEEDS` environment variable points to a valid YAML file with at least one feed configured.

### "Failed to fetch timeline: 401 Unauthorized"

**Solution**: Your OAuth token is invalid or expired. Get a new token and update `config.yml`.

### "Filtered out 0 posts but expected some"

**Solution**: Check that the DIDs in `blocked_reposters` are correct (they must start with `did:` and match the exact DID of the reposter).

### Posts not appearing in feed

**Possible causes**:
1. Poll interval too long - decrease `poll_interval`
2. Token expired - refresh OAuth token
3. PDS URL incorrect - verify `pds_url` is correct
4. Check logs with `RUST_LOG=debug` for errors

### High CPU/Memory usage

**Solutions**:
- Increase `poll_interval` to reduce API calls
- Decrease `max_posts_per_poll`
- Enable `CLEANUP_TASK_ENABLE` to remove old posts
- Reduce number of configured users

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

Copyright (c) 2025 n0vedad

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
