-- Migration: Create specialized tables for Images, Audio, and Video assets

CREATE TABLE IF NOT EXISTS image_assets (
    id TEXT NOT NULL PRIMARY KEY, -- UUID
    filename TEXT NOT NULL UNIQUE,
    identifier TEXT UNIQUE,
    file_path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    new_path TEXT, -- Optional optimized path
    bytes_size INTEGER NOT NULL,
    created_at DATETIME,
    modified_at DATETIME,
    -- Image Specifics
    width INTEGER,
    height INTEGER,
    alt_text TEXT
);

CREATE TABLE IF NOT EXISTS audio_assets (
    id TEXT NOT NULL PRIMARY KEY, -- UUID
    filename TEXT NOT NULL UNIQUE,
    identifier TEXT UNIQUE,
    file_path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    new_path TEXT,
    bytes_size INTEGER NOT NULL,
    created_at DATETIME,
    modified_at DATETIME,
    -- Audio Specifics
    bitrate_kbps INTEGER,
    duration_seconds INTEGER,
    sample_rate_hz INTEGER,
    channels INTEGER,
    codec TEXT
);

CREATE TABLE IF NOT EXISTS video_assets (
    id TEXT NOT NULL PRIMARY KEY, -- UUID
    filename TEXT NOT NULL UNIQUE,
    identifier TEXT UNIQUE,
    file_path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    new_path TEXT,
    bytes_size INTEGER NOT NULL,
    created_at DATETIME,
    modified_at DATETIME,
    -- Video Specifics
    duration_seconds INTEGER,
    width INTEGER,
    height INTEGER,
    frame_rate INTEGER,
    video_codec TEXT,
    audio_codec TEXT
);
