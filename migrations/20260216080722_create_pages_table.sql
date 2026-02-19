-- Add migration script here
CREATE TABLE IF NOT EXISTS pages (
    identifier          TEXT NOT NULL UNIQUE PRIMARY KEY,
    filename            TEXT NOT NULL UNIQUE,
    name                TEXT,
    html_content        TEXT NOT NULL,
    md_content          TEXT NOT NULL,
    md_content_hash     TEXT NOT NULL,
    tags                TEXT,
    modified_datetime   INTEGER,
    created_datetime    INTEGER
);
