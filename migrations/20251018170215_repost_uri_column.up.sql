-- Add repost_uri column to store the repost URI separately from the post URI
-- When is_repost=1:
--   uri = original post URI
--   repost_uri = repost record URI
-- When is_repost=0:
--   uri = post URI
--   repost_uri = NULL
ALTER TABLE feed_content ADD COLUMN repost_uri TEXT;
