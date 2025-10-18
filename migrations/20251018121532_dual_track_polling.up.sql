-- Add separate tracking for backfill polling
CREATE TABLE IF NOT EXISTS timeline_poll_backfill (
  user_did TEXT PRIMARY KEY NOT NULL,
  last_poll_at TEXT NOT NULL,
  posts_indexed INTEGER NOT NULL DEFAULT 0,
  total_posts_indexed INTEGER NOT NULL DEFAULT 0
);
