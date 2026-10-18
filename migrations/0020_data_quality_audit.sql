-- Admin data-quality queue: auditable actions (add answer, dismiss, remove track)
-- taken while reviewing tracks missing their room's expected answer type.

CREATE TABLE IF NOT EXISTS data_quality_actions (
    id BIGSERIAL PRIMARY KEY,
    track_id BIGINT,
    playlist_id BIGINT,
    room_id BIGINT,
    answer_type_id BIGINT,
    action VARCHAR(20) NOT NULL,
    value TEXT,
    payload JSONB,
    reviewer_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS data_quality_actions_track_idx
    ON data_quality_actions (track_id, room_id, action);

-- One active dismissal per (track, room, type).
CREATE UNIQUE INDEX IF NOT EXISTS data_quality_actions_dismiss_uq
    ON data_quality_actions (track_id, room_id, answer_type_id)
    WHERE action = 'dismiss';

CREATE INDEX IF NOT EXISTS data_quality_actions_created_idx
    ON data_quality_actions (created_at DESC);
