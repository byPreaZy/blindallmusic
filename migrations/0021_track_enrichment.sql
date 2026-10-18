-- External research evidence attached to tracks, surfaced in the
-- data-quality review queue (album, MusicBrainz work, dictionary match).

CREATE TABLE IF NOT EXISTS track_enrichment (
    track_id BIGINT PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    album TEXT,
    mb_work TEXT,
    mb_releases TEXT,
    dict_work TEXT,
    confidence VARCHAR(10),
    researched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
