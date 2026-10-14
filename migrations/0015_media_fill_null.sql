-- Pass 5 — fill MISSING media answers on media-category playlists.
-- NULL media means the room never asks the work for that track; titles with
-- explicit markers (From "X" / De "X" / aus "X" / B.O. "X" / BOF…) identify the
-- work with high confidence. Playlist → answer-type mapping mirrors the
-- existing data (Disney→Movie, Anime→Anime, Dessin animé→Cartoon, JV→Video
-- game, Séries→Show, Films→Movie, Films d'animations→Cartoon).
-- P2: fragment repairs (values mangled by earlier passes).
-- P3: curated single-title fills for marker-less tracks (Disney playlist).

-- P1 -------------------------------------------------------------------------
WITH src AS (
    SELECT DISTINCT ON (t.id, ptype.type_name)
        t.id AS track_id,
        ptype.type_name,
        btrim(regexp_replace(regexp_replace(
            coalesce(
                substring(m.m from '["“”‘’]([^"“”‘’]{2,80}?)\s*["“”‘’]'),
                substring(m.m from '["“”‘’]([^"“”‘’/]{2,80}?)\s*["“”‘’]?\s*$'),
                substring(m.m from $$''(.{2,80}?)''$$),
                substring(m.m from $$^\s*([^"“”'‘’/()\[\];]{2,120})\s*$$)
            ),
            $$(?i)^\s*(?:disney'?s|disney channel'?s|walt disney records|walt disney)\s+$$, ''),
            $$(?i)^["“”‘’'\s]+|["“”‘’']+\s*$$, '')) AS new_value
    FROM tracks t
    JOIN track_answers tt ON tt.track_id = t.id
    JOIN answer_types att ON att.id = tt.answer_type_id AND att.name = 'Title'
    JOIN (VALUES
        (8,'Movie'), (10,'Anime'), (16,'Cartoon'), (17,'Video game'),
        (18,'Show'), (22,'Movie'), (35,'Cartoon')
    ) AS ptype(pid, type_name) ON ptype.pid = t.playlist_id
    CROSS JOIN LATERAL (
        SELECT substring(tt.value from $$(?i)[\(\[]\s*(?:(?:chanson|musique|titre|song|track|theme|g[ée]n[ée]rique|opening|ending|intro|symphonic suite|suite|main title|end title|medley|version|bande originale|soundtrack)s?\s+)?(?:de|d'|du|des|from|aus|von|extrait(?:e|s)? d[ue]|issu[es]? d[ue]|tir[ée][es]? d[ue]|bof|b\.o\.|bo|bande originale|theme song from|theme from|main title|end title|music from|songs? from|featured in|hauptthema|thema|dessin anim[ée]|film|de la bande originale|de la s[ée]rie|du film|du dessin anim[ée]|original(?:e)? d[ue])\s*[:]?\s*([^()\[\]]{2,160})$$) AS m
    ) m
    WHERE m.m IS NOT NULL
      AND NOT EXISTS (
          SELECT 1 FROM track_answers ta
          JOIN answer_types at ON at.id = ta.answer_type_id
          WHERE ta.track_id = t.id
            AND at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical'))
    ORDER BY t.id, ptype.type_name, length(m.m) DESC
)
INSERT INTO track_answers (track_id, answer_type_id, value, score, created_at, updated_at)
SELECT src.track_id, at.id,
       btrim(regexp_replace(src.new_value,
           $$(?i)\s*:\s*(?:season|vol\.?|volume|part|episode|chapter)s?\s*\d.*$$, '')),
       0.5, now(), now()
FROM src
JOIN answer_types at ON at.name = src.type_name
WHERE length(src.new_value) >= 2
  -- extracted junk guards
  AND src.new_value !~* '^(de|du|des|d''|of|the|fin|final|finale|générique|version|soundtrack|bande originale)\b'
  AND src.new_value !~* '^(instrumental|version|remix|live|demo|acoustic|karaoke|pop|single|edit|mono|stereo|extended|main title|end title|prologue|epilogue|score|medley|theme|intro|outro|interlude|prelude|overture|bonus|deleted|alternate|original|soundtrack|film|movie|radio|short|long|full|complete|part|vol\.?|disc|cd|act|scene|clip|video|official|lyric|doublage|french|english|german|deutsch|cast|ensemble|orchestra|chorus|choeur|suite|symphonic|concert|deluxe|edition|remaster(ed)?|19\d\d|20\d\d)\b';

-- P2 -------------------------------------------------------------------------
-- Fragments mangled by earlier extraction passes.
UPDATE track_answers ta
SET value = 'Vaiana', updated_at = now()
FROM answer_types at
WHERE at.id = ta.answer_type_id AND at.name = 'Movie'
  AND ta.value::text IN ('du Bout du Monde', 'du bout du monde');

-- "Aller plus loin (Version générique de fin)" — Cerise Calixte is Vaiana's
-- voice; this is the Vaiana 2 (Beyond) end-credits FR version.
UPDATE track_answers ta
SET value = 'Vaiana 2', updated_at = now()
FROM answer_types at
WHERE at.id = ta.answer_type_id AND at.name = 'Movie'
  AND ta.value::text = 'générique de fin';

-- "La Ronde des Animaux" Disney compilation songs.
UPDATE track_answers ta
SET value = 'La Ronde des Animaux', updated_at = now()
FROM answer_types at
WHERE at.id = ta.answer_type_id AND at.name = 'Movie'
  AND ta.value::text = 'Des Animaux';

UPDATE track_answers ta
SET value = 'Moana', updated_at = now()
FROM answer_types at
WHERE at.id = ta.answer_type_id AND at.name = 'Movie'
  AND ta.value::text = 'Moana Karaoke';

UPDATE track_answers ta
SET value = 'Winnie the Pooh',
    aliases = coalesce(ta.aliases, '[]'::jsonb) || '["Winnie l''Ourson","Les Aventures de Winnie l''Ourson","Winnie Puuh"]'::jsonb,
    updated_at = now()
FROM answer_types at
WHERE at.id = ta.answer_type_id AND at.name = 'Movie'
  AND ta.value::text = 'Die vielen Lieder von Winnie Puuh';

-- P3 -------------------------------------------------------------------------
-- Curated fills for marker-less titles (Disney playlist track ids).
CREATE TEMP TABLE _media_fill(track_id bigint, media_value citext, add_aliases jsonb);
INSERT INTO _media_fill VALUES
 (12529, 'Beauty and the Beast', '["La Belle et la Bête","A Bela e a Fera"]'),
 (12603, 'Brother Bear',         '["Mon frère l''ours","Bärenbrüder","Frère des Ours"]'),
 (12031, 'Beauty and the Beast', '["La Belle et la Bête"]'),
 (12200, 'Finding Nemo',         '["Le Monde de Nemo","Finding Nemo"]'),
 (12165, 'ZOMBIES 4: Dawn of the Vampires', '["ZOMBIES 4","Zombies 4"]'),
 (11979, 'Enchanted',            '["Il était une fois","Disenchanted"]'),
 (12611, 'Wish',                 '["Wish: Asha et la bonne étoile"]'),
 (4948,  'Cars',                 '["Cars","Les Bagnoles","Quatre Roues"]'),
 (12569, 'Hannah Montana',       '[]'),
 (11629, 'Alice au Pays des Merveilles', '["Alice in Wonderland"]'),
 (11927, 'Planes',               '[]'),
 (12161, 'ZOMBIES 4: Dawn of the Vampires', '["ZOMBIES 4","Zombies 4"]'),
 (12135, 'Starstruck',           '["Starstruck: Rencontre avec une star"]'),
 (12590, 'Elemental',            '["Élémentaire"]'),
 (12545, 'Winnie the Pooh',      '["Winnie l''Ourson"]'),
 (12167, 'ZOMBIES 4: Dawn of the Vampires', '["ZOMBIES 4","Zombies 4"]');

INSERT INTO track_answers (track_id, answer_type_id, value, score, aliases, created_at, updated_at)
SELECT mf.track_id,
       (SELECT id FROM answer_types WHERE name = 'Movie'),
       mf.media_value, 0.5,
       NULLIF(mf.add_aliases, '[]'::jsonb),
       now(), now()
FROM _media_fill mf
WHERE EXISTS (SELECT 1 FROM tracks t WHERE t.id = mf.track_id)
  AND NOT EXISTS (
      SELECT 1 FROM track_answers ta
      JOIN answer_types at ON at.id = ta.answer_type_id
      WHERE ta.track_id = mf.track_id
        AND at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical'));

DROP TABLE _media_fill;
