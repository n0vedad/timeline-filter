-- Timeline Filter: Add tables for per-user timeline filtering

-- User configuration table
-- Stores OAuth credentials and feed settings for each user
CREATE TABLE timeline_user_config (
  did TEXT PRIMARY KEY,
  feed_uri TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  description TEXT NOT NULL,

  -- OAuth credentials
  access_token TEXT NOT NULL,
  refresh_token TEXT,
  token_expires_at TEXT,  -- ISO 8601 format
  pds_url TEXT NOT NULL,  -- e.g. "https://bsky.social"

  -- Polling configuration
  poll_interval_seconds INTEGER NOT NULL DEFAULT 30,
  max_posts_per_poll INTEGER NOT NULL DEFAULT 50,

  -- Metadata
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_timeline_user_config_feed_uri ON timeline_user_config(feed_uri);

-- User filter rules (normalized table for flexibility)
-- Stores filter rules like blocked reposters, blocked authors, etc.
CREATE TABLE timeline_user_filters (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_did TEXT NOT NULL,
  filter_type TEXT NOT NULL,  -- 'blocked_reposter', 'blocked_author', etc.
  filter_value TEXT NOT NULL,  -- DID or keyword
  created_at TEXT NOT NULL DEFAULT (datetime('now')),

  FOREIGN KEY (user_did) REFERENCES timeline_user_config(did) ON DELETE CASCADE,
  UNIQUE(user_did, filter_type, filter_value)
);

CREATE INDEX idx_timeline_user_filters_user ON timeline_user_filters(user_did);
CREATE INDEX idx_timeline_user_filters_type ON timeline_user_filters(filter_type, filter_value);

-- Poll cursor and state tracking
-- Tracks the last poll time and cursor position for each user
CREATE TABLE timeline_poll_cursor (
  user_did TEXT PRIMARY KEY,
  last_cursor TEXT,  -- Cursor from last getTimeline() call
  last_poll_at TEXT NOT NULL,  -- ISO 8601 format
  last_indexed_at TEXT,  -- ISO 8601 of most recent post indexed
  posts_indexed INTEGER NOT NULL DEFAULT 0,  -- Count of posts indexed in last poll
  total_posts_indexed INTEGER NOT NULL DEFAULT 0,  -- Total posts ever indexed

  FOREIGN KEY (user_did) REFERENCES timeline_user_config(did) ON DELETE CASCADE
);

CREATE INDEX idx_timeline_poll_cursor_last_poll ON timeline_poll_cursor(last_poll_at);
