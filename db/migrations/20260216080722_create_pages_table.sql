CREATE TABLE IF NOT EXISTS pages (
    identifier          TEXT NOT NULL UNIQUE PRIMARY KEY,
    filename            TEXT NOT NULL UNIQUE,
    name                TEXT,
    md_content          TEXT NOT NULL,
    content_hash        TEXT NOT NULL,
    tags                TEXT,
    modified_datetime   TEXT,
    created_datetime    TEXT,
    file_path           TEXT NOT NULL,
    new_path            TEXT
);
