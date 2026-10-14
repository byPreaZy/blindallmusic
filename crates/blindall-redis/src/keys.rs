pub const ROUND_TTL_SECONDS: i64 = 3600;
pub const PRESENCE_TTL_SECONDS: i64 = 7200;
pub const PRESENCE_PRUNE_SECONDS: i64 = 90;

pub fn round_players(round_id: i64) -> String {
    format!("round:{round_id}:players")
}

pub fn round_user_score(round_id: i64, user_id: i64) -> String {
    format!("round:{round_id}:scores:{user_id}")
}

pub fn round_podium(round_id: i64) -> String {
    format!("round:{round_id}:podium")
}

pub fn round_user_answers(round_id: i64, user_id: i64, track_id: i64) -> String {
    format!("round:{round_id}:answers:{user_id}:{track_id}")
}

pub fn round_answer_players(round_id: i64, answer_id: i64) -> String {
    format!("round:{round_id}:answer:{answer_id}:players")
}

pub fn round_user_tracks(round_id: i64, user_id: i64) -> String {
    format!("round:{round_id}:tracks:{user_id}")
}

pub fn room_players(room_id: i64) -> String {
    format!("room:{room_id}:players")
}

pub fn room_members(room_id: i64) -> String {
    format!("room:{room_id}:members_z")
}
