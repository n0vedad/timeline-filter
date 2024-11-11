-- Add up migration script here

ALTER TABLE feed_content ADD COLUMN score INT NOT NULL DEFAULT 0;
