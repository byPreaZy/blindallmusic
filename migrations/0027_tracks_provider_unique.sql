-- 0027: (playlist_id, provider, provider_id) is the natural key of an
-- imported track — PHP relies on ProcessImportTrack::doesntExist() + the
-- SELECT-then-INSERT in upsert_track left room for duplicates under
-- concurrent imports. Merge duplicates, then enforce uniqueness.

CREATE TEMP TABLE _dup_tracks AS
SELECT id,
       MIN(id) OVER (PARTITION BY playlist_id, provider, provider_id) AS keep_id
FROM tracks;
CREATE INDEX ON _dup_tracks (id);

-- Move answers from duplicate rows onto the keeper when the keeper lacks
-- that answer type; leftovers are removed by the cascade on track delete.
UPDATE track_answers ta SET track_id = d.keep_id
FROM _dup_tracks d
WHERE ta.track_id = d.id AND d.id <> d.keep_id
  AND NOT EXISTS (
      SELECT 1 FROM track_answers k
      WHERE k.track_id = d.keep_id AND k.answer_type_id = ta.answer_type_id
  );

UPDATE scores s SET track_id = d.keep_id
FROM _dup_tracks d
WHERE s.track_id = d.id AND d.id <> d.keep_id;

DELETE FROM tracks t USING _dup_tracks d
WHERE t.id = d.id AND d.id <> d.keep_id;

DROP TABLE _dup_tracks;

CREATE UNIQUE INDEX IF NOT EXISTS tracks_playlist_provider_uidx
    ON tracks (playlist_id, provider, provider_id);
