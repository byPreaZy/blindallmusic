-- Prevent concurrent ensure_current() calls from creating duplicate seasons
-- for the same window.
DELETE FROM seasons a USING seasons b
WHERE a.starts_at = b.starts_at AND a.ends_at = b.ends_at AND a.id > b.id;

CREATE UNIQUE INDEX IF NOT EXISTS seasons_name_uidx ON seasons (name);
CREATE UNIQUE INDEX IF NOT EXISTS seasons_starts_uidx ON seasons (starts_at);
