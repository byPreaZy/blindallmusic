-- Weekly competitive seasons: a rolling Mon→Mon (UTC) window with a snapshot
-- podium and XP rewards granted at finalization.

CREATE TABLE IF NOT EXISTS seasons (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ NOT NULL,
    podium JSONB,
    rewards_granted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS seasons_window_idx ON seasons (starts_at, ends_at);
CREATE INDEX IF NOT EXISTS seasons_ends_idx ON seasons (ends_at);

ALTER TABLE user_levels
    ADD COLUMN IF NOT EXISTS season_bonus_xp BIGINT NOT NULL DEFAULT 0;
