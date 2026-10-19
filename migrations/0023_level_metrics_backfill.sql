-- Backfill motivational metrics that were never written (matches the
-- LevelTrigger::Score computation added in levels.rs).
UPDATE user_levels ul SET
    rounds_played_count = COALESCE((
        SELECT COUNT(*) FROM round_standings rs WHERE rs.user_id = ul.user_id), 0),
    unique_rooms_played_count = COALESCE((
        SELECT COUNT(DISTINCT rs.room_id) FROM round_standings rs
        WHERE rs.user_id = ul.user_id), 0),
    best_round_score = COALESCE((
        SELECT MAX(rs.total_score) FROM round_standings rs
        WHERE rs.user_id = ul.user_id), 0),
    correct_answers_count = COALESCE((
        SELECT COUNT(*) FROM scores s WHERE s.user_id = ul.user_id AND s.score > 0), 0);
