-- Remove stats tracking columns
ALTER TABLE feed_content DROP COLUMN is_repost;
ALTER TABLE timeline_poll_cursor DROP COLUMN blocked_posts_count;
