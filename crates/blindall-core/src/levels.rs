#[derive(Debug, Clone, Copy, Default)]
pub struct XpInputs {
    pub score_public_rooms: i64,
    pub minigame_scores_total: i64,
    pub months_seniority: i64,
    pub rooms_created_count: i64,
    pub has_team: bool,
    pub playlists_created_count: i64,
    pub tracks_liked_count: i64,
    pub consecutive_days_streak: i64,
    pub season_bonus_xp: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelData {
    pub level: i64,
    pub total_xp: i64,
    pub current_xp: i64,
    pub xp_for_next_level: i64,
}

pub fn compute_level(i: &XpInputs) -> LevelData {
    let total_xp = i.score_public_rooms
        + i.minigame_scores_total.min(1000)
        + (i.months_seniority * 50).min(600)
        + (i.rooms_created_count * 100).min(1000)
        + if i.has_team { 200 } else { 0 }
        + (i.playlists_created_count * 20).min(2000)
        + (i.tracks_liked_count * 5).min(1000)
        + (i.consecutive_days_streak * 10).min(300)
        + i.season_bonus_xp.min(5000);

    level_from_total_xp(total_xp)
}

pub fn level_from_total_xp(total_xp: i64) -> LevelData {
    let mut level = 1i64;
    let mut xp_for_current = 0i64;
    let mut xp_for_next = 100i64;
    let xp_increment = 50i64;

    while total_xp >= xp_for_next {
        xp_for_current = xp_for_next;
        level += 1;
        xp_for_next = xp_for_current + (100 + (level - 2) * xp_increment);
    }

    LevelData {
        level,
        total_xp,
        current_xp: (total_xp - xp_for_current).max(0),
        xp_for_next_level: xp_for_next - xp_for_current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_match_php_code() {
        // NOTE: the PHP code contradicts its own doc-comment, the loop
        assert_eq!(level_from_total_xp(0).level, 1);
        assert_eq!(level_from_total_xp(99).level, 1);
        assert_eq!(level_from_total_xp(100).level, 2);
        assert_eq!(level_from_total_xp(199).level, 2);
        assert_eq!(level_from_total_xp(200).level, 3);
        assert_eq!(level_from_total_xp(349).level, 3);
        assert_eq!(level_from_total_xp(350).level, 4);
        assert_eq!(level_from_total_xp(550).level, 5);
    }

    #[test]
    fn caps_applied() {
        let d = compute_level(&XpInputs {
            score_public_rooms: 0,
            minigame_scores_total: 99999, // capped 1000
            months_seniority: 100,        // capped 600
            rooms_created_count: 100,     // capped 1000
            has_team: true,               // +200
            playlists_created_count: 500, // capped 2000
            tracks_liked_count: 500,      // capped 1000
            consecutive_days_streak: 100, // capped 300
            season_bonus_xp: 0,
        });
        assert_eq!(d.total_xp, 1000 + 600 + 1000 + 200 + 2000 + 1000 + 300);
    }
}
