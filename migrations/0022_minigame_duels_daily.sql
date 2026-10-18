-- Async minigame duels: fixed question sets generated at creation so both
-- players answer the identical sequence. Daily challenges share the same
-- lazily generated set per (day, game_type).

CREATE TABLE IF NOT EXISTS minigame_duels (
    id BIGSERIAL PRIMARY KEY,
    game_type VARCHAR(30) NOT NULL,
    creator_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    opponent_id BIGINT REFERENCES users (id) ON DELETE CASCADE,
    questions JSONB NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'open',
    creator_done BOOLEAN NOT NULL DEFAULT false,
    opponent_done BOOLEAN NOT NULL DEFAULT false,
    creator_score INT NOT NULL DEFAULT 0,
    opponent_score INT NOT NULL DEFAULT 0,
    creator_answers JSONB NOT NULL DEFAULT '[]'::jsonb,
    opponent_answers JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS minigame_duels_creator_idx
    ON minigame_duels (creator_id, status);
CREATE INDEX IF NOT EXISTS minigame_duels_opponent_idx
    ON minigame_duels (opponent_id, status);
CREATE INDEX IF NOT EXISTS minigame_duels_open_idx
    ON minigame_duels (status, created_at) WHERE status = 'open';

CREATE TABLE IF NOT EXISTS minigame_daily (
    day DATE NOT NULL,
    game_type VARCHAR(30) NOT NULL,
    questions JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (day, game_type)
);

CREATE TABLE IF NOT EXISTS minigame_daily_scores (
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    day DATE NOT NULL,
    game_type VARCHAR(30) NOT NULL,
    score INT NOT NULL DEFAULT 0,
    answers JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, day, game_type)
);
