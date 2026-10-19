-- Performance indexes identified by the DB audit: FKs and hot-path filters
-- that were doing sequential scans on growing tables. Plain CREATE INDEX
-- (not CONCURRENTLY): sqlx runs each migration inside a transaction.

CREATE INDEX IF NOT EXISTS rounds_room_id_idx ON rounds (room_id);
CREATE INDEX IF NOT EXISTS tracks_playlist_id_idx ON tracks (playlist_id);
CREATE INDEX IF NOT EXISTS tracks_provider_idx ON tracks (provider, provider_id);
CREATE INDEX IF NOT EXISTS playlist_room_room_id_idx ON playlist_room (room_id);
CREATE INDEX IF NOT EXISTS playlist_room_playlist_id_idx ON playlist_room (playlist_id);

CREATE INDEX IF NOT EXISTS users_elo_active_idx ON users (elo DESC, id)
    WHERE deleted_at IS NULL AND is_guest = false;
CREATE INDEX IF NOT EXISTS users_provider_idx ON users (provider, provider_id);
CREATE INDEX IF NOT EXISTS user_levels_level_xp_idx ON user_levels (level DESC, total_xp DESC);

CREATE INDEX IF NOT EXISTS round_standings_created_at_idx ON round_standings (created_at);
CREATE INDEX IF NOT EXISTS round_standings_team_id_idx ON round_standings (team_id);
CREATE INDEX IF NOT EXISTS round_standings_room_user_idx ON round_standings (room_id, user_id);

CREATE INDEX IF NOT EXISTS notifications_unread_idx ON notifications (notifiable_id, created_at DESC)
    WHERE read_at IS NULL AND notifiable_type = 'App\Models\User';

CREATE INDEX IF NOT EXISTS moderables_user_idx ON moderables (user_id, moderable_type);
CREATE INDEX IF NOT EXISTS bookmarks_user_idx ON bookmarks (user_id, bookmarkable_type);
CREATE INDEX IF NOT EXISTS messages_room_id_idx ON messages (messagable_type, messagable_id, id DESC);
CREATE INDEX IF NOT EXISTS messages_user_idx ON messages (user_id);
CREATE INDEX IF NOT EXISTS scores_track_id_idx ON scores (track_id);
CREATE INDEX IF NOT EXISTS rooms_user_id_idx ON rooms (user_id);
CREATE INDEX IF NOT EXISTS playlists_user_id_idx ON playlists (user_id);
CREATE INDEX IF NOT EXISTS team_requests_user_idx ON team_requests (user_id);

-- Trigram indexes backing the LIKE '%…%' searches (room/track-answer/user).
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX IF NOT EXISTS track_answers_value_trgm ON track_answers USING gin (value gin_trgm_ops);
CREATE INDEX IF NOT EXISTS users_name_trgm ON users USING gin (name gin_trgm_ops);
