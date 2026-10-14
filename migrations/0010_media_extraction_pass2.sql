-- Pass 2 media-answer normalization.
-- Pass 1 (0009) only handled titles ending in (De "X"). This pass covers
-- (from "X"), (from 'X'), (from X), (aus "X"), (extrait de X), (BO de X),
-- (theme song from X)… and, as a second pass, derives the work name from
-- compilation-like media values that embed it (": The Lion King", "(Songs from
-- Naruto)", "X, Vol. 2", "X: Season 1").

-- ---------------------------------------------------------------------------
-- Pass A: extract work name from the track title marker.
-- Order matters: double/typographic quotes first (handles preambles such as
-- "DreamWorks Animation's "TROLLS""), then single quotes, then bare names.
-- ---------------------------------------------------------------------------
WITH extracted AS (
    SELECT
        ta.id AS answer_id,
        regexp_replace(
            coalesce(
                substring(tt.value from '\((?:de|from|aus|extrait(?:e)? d[ue]|bo d[ue]|bande originale(?: du| de| sonore)?|version d[ue]|theme song from|titre d[ue]|music from|songs? from|featured in|générique d[ue]|hauptthema (?:aus|von)|thema (?:aus|von))[^"“”()]*["“]([^"“”]{2,80})["”]'),
                substring(tt.value from '\((?:de|from|aus|extrait(?:e)? d[ue]|bo d[ue]|bande originale(?: du| de| sonore)?|version d[ue]|theme song from|titre d[ue]|music from|songs? from|featured in|générique d[ue]|hauptthema (?:aus|von)|thema (?:aus|von))\s+''([^'']{2,80})'''),
                substring(tt.value from '\((?:de|from|aus|extrait(?:e)? d[ue]|bo d[ue]|bande originale(?: du| de| sonore)?|version d[ue]|theme song from|titre d[ue]|music from|songs? from|featured in|générique d[ue]|hauptthema (?:aus|von)|thema (?:aus|von))\s+([^"“”''()/]{2,80})')
            ),
            -- cleanup of trailer noise on bare/quoted captures
            '\s*(\[[^\]]*\]|\(|\bMotion Picture\b.*|\bOriginal Motion Picture\b.*|\bSoundtrack\b.*|\bScore\b.*|\bDeutscher\b.*|\bDeutsche\b.*|\bFilm-Soundtrack\b.*|\bBande Originale\b.*|\bB\.?O\.?F\.?\b.*|\bOST\b.*|\bSingle Version\b.*|\bVersion\b.*|,\s*(Vol\.?|Volume|Part)\b.*|\s+-\s+(Vol\.?|Volume|Part|Score|Soundtrack)\b.*)$',
            '',
            'i'
        ) AS new_value,
        ta.value AS old_value
    FROM track_answers ta
    JOIN answer_types at ON at.id = ta.answer_type_id
    JOIN track_answers tt ON tt.track_id = ta.track_id
    JOIN answer_types att ON att.id = tt.answer_type_id AND att.name = 'Title'
    WHERE at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
      AND tt.value ~* '\((de|from|aus|extrait|bo |bande originale|version|theme song|titre|music from|songs? from|featured in|générique|hauptthema|thema )'
)
UPDATE track_answers ta
SET value = ex.new_value, updated_at = now()
FROM extracted ex
WHERE ta.id = ex.answer_id
  AND length(btrim(ex.new_value)) >= 2
  AND lower(btrim(ex.new_value)) <> lower(btrim(ta.value))
  AND lower(btrim(ta.value)) NOT LIKE '%' || lower(btrim(ex.new_value)) || '%';

-- ---------------------------------------------------------------------------
-- Pass B: for media answers still pointing at a compilation, derive the work
-- name from the value itself when it embeds one.
-- ---------------------------------------------------------------------------
WITH derived AS (
    SELECT
        ta.id AS answer_id,
        btrim(coalesce(
            -- "(Songs from X)" / "(From X)" inside the value
            substring(ta.value from '\((?:songs?|music|themes?|titres?|chansons?) (?:from|de|du|des|aus|of)\s+"?([^"()]{2,60})'),
            -- "… Collection: X" / "… Records: X" → X
            CASE WHEN ta.value ~* '(collection|records|legacy|songs?|chansons?|album|best of|soundtrack|music|hits)'
                 THEN substring(ta.value from ':\s*([^:()]{2,60})$') END,
            -- "X: Season n …" / "X - Vol. n" / "X, Vol. n" → X
            CASE WHEN ta.value ~* '(:\s*Season|,\s*Vol\.?|\s+-\s+Vol\.?|,\s*Volume|\s+Volume\s+\d)'
                 THEN substring(ta.value from '^([^:,()]{2,60}?)\s*(?::\s*Season|,\s*Vol\.?|\s+-\s+Vol\.?|,\s*Volume|\s+Volume\s+\d)') END
        )) AS new_value
    FROM track_answers ta
    JOIN answer_types at ON at.id = ta.answer_type_id
    WHERE at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
      AND ta.value ~* '(disney|chansons?|collection|vol\.?|hits|classics?|princesses?|popcorn|we love|favorites|celebration|nation|wave|cinema|beats|skies|moments?|magic|wedding|junior|animaux|romance|superstar|karaoke|piano|orchestra|symphon|covers?|tribute|greatest|best of|anthology|essential|soundtrack|legacy|season|beneath|dance)'
)
UPDATE track_answers ta
SET value = ex.new_value, updated_at = now()
FROM derived ex
WHERE ta.id = ex.answer_id
  AND ex.new_value IS NOT NULL
  AND length(ex.new_value) >= 2
  -- refuse generic leftovers that are still compilations
  AND ex.new_value !~* '^(disney|chansons?|collection|hits|classics?|princesses?|popcorn|moment popcorn|we love|weddings?|junior|animaux|piano|orchestra|season|volume|vol\.?|part|disc|the|la |les |le |de |des |du |a |an )'
  AND lower(ex.new_value) <> lower(btrim(ta.value));

-- ---------------------------------------------------------------------------
-- Pass C: curated fixes for known offenders observed in production testing.
-- ---------------------------------------------------------------------------
UPDATE track_answers ta SET value = 'Le Roi Lion', aliases = '["Le Roi Lion 3","Le Roi Lion 1½","The Lion King","Hakuna Matata"]'::jsonb, updated_at = now()
WHERE ta.value = 'Pure Disney'
  AND EXISTS (SELECT 1 FROM track_answers tt WHERE tt.track_id = ta.track_id AND tt.value ILIKE '%Digga Tunnah%');

UPDATE track_answers ta SET value = 'Cendrillon', aliases = '["Cinderella"]'::jsonb, updated_at = now()
WHERE ta.value = 'Princesses: Les Plus Belles Chansons'
  AND EXISTS (SELECT 1 FROM track_answers tt WHERE tt.track_id = ta.track_id AND tt.value ILIKE '%Tendre Rêve%');

UPDATE track_answers ta SET value = 'Blanche-Neige et les Sept Nains', aliases = '["Blanche Neige","Snow White","Snow White and the Seven Dwarfs"]'::jsonb, updated_at = now()
WHERE ta.value = 'La Magie De Disney'
  AND EXISTS (SELECT 1 FROM track_answers tt WHERE tt.track_id = ta.track_id AND tt.value ILIKE '%mon prince viendra%');

UPDATE track_answers ta SET value = 'Pocahontas', aliases = '["Pocahontas - Une Légende Indienne","Pocahontas: Une légende indienne"]'::jsonb, updated_at = now()
WHERE ta.value = 'Disney Classics'
  AND EXISTS (SELECT 1 FROM track_answers tt WHERE tt.track_id = ta.track_id AND tt.value ILIKE '%Colours of the Wind%');

UPDATE track_answers ta SET value = 'La Belle et la Bête', aliases = '["Beauty and the Beast"]'::jsonb, updated_at = now()
WHERE ta.value = 'Jim Brickman - The Disney Songbook'
  AND EXISTS (SELECT 1 FROM track_answers tt WHERE tt.track_id = ta.track_id AND tt.value ILIKE '%Beauty and the Beast%');

UPDATE track_answers ta SET value = 'Les Aventures de Winnie l''Ourson', aliases = '["Winnie l''Ourson","Winnie the Pooh"]'::jsonb, updated_at = now()
WHERE ta.value = 'Magical Moments 2: The Magic Continues'
  AND EXISTS (SELECT 1 FROM track_answers tt WHERE tt.track_id = ta.track_id AND tt.value ILIKE '%Winnie the Pooh%');

UPDATE track_answers ta SET value = 'Le Roi Lion', aliases = '["The Lion King"]'::jsonb, updated_at = now()
WHERE ta.value = 'Disney''s Fairy Tale Weddings'
  AND EXISTS (SELECT 1 FROM track_answers tt WHERE tt.track_id = ta.track_id AND tt.value ILIKE '%Can You Feel the Love Tonight%');

UPDATE track_answers ta SET value = 'Girl Meets World', aliases = '["Take On the World"]'::jsonb, updated_at = now()
WHERE ta.value = 'Take On the World'
  AND EXISTS (SELECT 1 FROM track_answers tt WHERE tt.track_id = ta.track_id AND tt.value ILIKE '%Take On the World%');
