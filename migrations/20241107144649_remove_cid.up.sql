-- Add up migration script here

DROP INDEX feed_content_idx_feed;
ALTER TABLE feed_content DROP COLUMN cid;
CREATE INDEX feed_content_idx_feed ON feed_content(feed_id, indexed_at DESC);
