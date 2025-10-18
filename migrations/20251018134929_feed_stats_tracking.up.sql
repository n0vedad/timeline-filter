-- Add is_repost column to feed_content to track reposts
ALTER TABLE feed_content ADD COLUMN is_repost BOOLEAN NOT NULL DEFAULT 0;

-- Add blocked_posts_count to timeline_poll_cursor to track blocked posts
ALTER TABLE timeline_poll_cursor ADD COLUMN blocked_posts_count INTEGER NOT NULL DEFAULT 0;
