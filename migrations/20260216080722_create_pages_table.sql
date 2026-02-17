-- Add migration script here
CREATE TABLE IF NOT EXISTS pages (
    filename            TEXT NOT NULL UNIQUE PRIMARY KEY,
    name                TEXT,
    html_content        TEXT NOT NULL,
    md_content          TEXT NOT NULL,
    tags                TEXT,
    modified_datetime   INTEGER,
    created_datetime    INTEGER
);
