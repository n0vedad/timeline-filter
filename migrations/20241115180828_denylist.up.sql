-- Add up migration script here

CREATE TABLE denylist (
    subject TEXT NOT NULL,
    reason TEXT NOT NULL,
    updated_at DATETIME NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (subject)
);

