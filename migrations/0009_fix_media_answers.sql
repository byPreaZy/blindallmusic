-- Répare les réponses "œuvre" (Movie/Cartoon/Anime/Show/Video game/Musical)
-- polluées par le nom de l'album de compilation : quand le titre annonce
-- (De "X"), la réponse attendue est X, pas le nom de la compile.

WITH src AS (
    SELECT ta.track_id,
           btrim(regexp_replace(
               ta.value,
               $$^.*[(]de[[:space:]]+["']?([^"'”)/]+)["'”)]?.*$$,
               '\1',
               'i'
           )) AS work
    FROM track_answers ta
    JOIN answer_types at ON at.id = ta.answer_type_id AND at.name = 'Title'
    WHERE ta.value ~* $$[(]de[[:space:]]+$$
)
UPDATE track_answers m
SET value = s.work
FROM src s
WHERE m.track_id = s.track_id
  AND m.answer_type_id IN (
      SELECT id FROM answer_types
      WHERE name IN ('Movie', 'Cartoon', 'Anime', 'Show', 'Video game', 'Musical')
  )
  AND lower(btrim(m.value)) <> lower(btrim(s.work));
