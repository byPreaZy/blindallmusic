-- =============================================================================
-- 0007 — Index manquant sur track_answers.track_id.
--
-- Le gameplay charge les réponses d'un extrait par `WHERE track_id = ?` à
-- chaque round, et l'enrichissement Deezer lit artiste/titre/featuring via
-- des sous-requêtes corrélées sur cette colonne. Sans index, chaque lookup
-- est un seq scan complet de la table (~160k lignes) — prohibitif en lot.
-- =============================================================================

CREATE INDEX IF NOT EXISTS track_answers_track_id_index
    ON track_answers (track_id);
