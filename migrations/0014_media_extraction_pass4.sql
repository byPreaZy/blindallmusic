-- Pass 4 media-answer normalization — all playlists (Jeux vidéo, Anime,
-- Films, Séries, Dessin animé leftovers).
--
-- P1: title-marker extraction rerun with an optional generic-noun prefix inside
--   the paren: (Chanson issue de "X"), (Symphonic Suite "X" / Live),
--   (Movie Main Theme)… Only applied where the current media is junk.
-- P2: derivation from the media VALUE itself:
--     a) "X: Season 2 - Vol. 2 (Chapters 13-16)" → X   (Mandalorian, Loki…)
--     b) "Star Wars: The Bad Batch - Vol. 1" → "Star Wars: The Bad Batch"
--     c) "…from X" inside value → X   (Song Collection from The Rose of Versailles)
--     d) "X SONG/VOCAL/SOUND COLLECTION …" → X   (Cardcaptor Sakura…)
-- P3: whitelisted junk values where the track title IS the work name
--     (cover albums like "Classic Cartoon Theme" list songs by show name).
-- P4: curated title → media map for residual compilations.

-- P1 -------------------------------------------------------------------------
WITH ex AS (
    SELECT DISTINCT ON (ta.id)
        ta.id AS answer_id,
        btrim(regexp_replace(
            regexp_replace(
                coalesce(
                    substring(m.m from '["“”]([^"“”]{2,80}?)\s*["“”]'),
                    substring(m.m from '["“”]([^"“”/]{2,80}?)\s*["“”]?\s*$'),
                    substring(m.m from $$''(.{2,80}?)''$$),
                    substring(m.m from $$^\s*([^"“”'/()\[\];]{2,120})\s*$$)
                ),
                $$(?i)^\s*(?:disney'?s|walt disney records|walt disney)\s+$$, ''),
            $$(?i)^["“”'\s]+|["“”']+\s*$$, '')) AS new_value
    FROM track_answers ta
    JOIN answer_types at ON at.id = ta.answer_type_id
    JOIN track_answers tt ON tt.track_id = ta.track_id
    JOIN answer_types att ON att.id = tt.answer_type_id AND att.name = 'Title'
    CROSS JOIN LATERAL (
        SELECT substring(tt.value from $$(?i)[\(\[]\s*(?:(?:chanson|musique|titre|song|track|theme|g[ée]n[ée]rique|opening|ending|intro|symphonic suite|suite|main title|end title|medley|version|bande originale|soundtrack)s?\s+)?(?:de|d'|du|des|from|aus|von|extrait(?:e|s)? d[ue]|issu[es]? d[ue]|tir[ée][es]? d[ue]|bof|bo|bande originale|version|theme song from|theme from|main title|end title|titre|music from|songs? from|featured in|g[ée]n[ée]rique|hauptthema|thema|dessin anim[ée]|film|soundtrack|de la bande originale|de la s[ée]rie|du film|du dessin anim[ée]|original(?:e)? d[ue])\s*[:]?\s*([^()\[\]]{2,160})$$) AS m
    ) m
    WHERE at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
      AND m.m IS NOT NULL
      AND ta.value ~* '(classic|collection|songbook|princesses|piano|wedding|magie|animaux|ronde|legacy|chansons|songs|giraffenaffen|playhouse|villains|karaoke|sing|tribute|covers|orchestr|playlist|compilation|favorites|greatest|best of|official|resort|parks|ukulele|lullaby|story|mania|renditions|volume|vol\.|season|episodes|chapters|suite|remix|remaster|version|album|gold|platinum|ultimate|anthology|essentials|complete|hits|popcorn|m[ée]lodies|monde|world)'
    ORDER BY ta.id, length(m.m) DESC
)
UPDATE track_answers ta
SET value = ex.new_value, updated_at = now()
FROM ex
WHERE ta.id = ex.answer_id
  AND length(ex.new_value) >= 2
  AND lower(btrim(ex.new_value)) <> lower(btrim(ta.value))
  AND lower(btrim(ta.value)) NOT LIKE '%' || lower(btrim(ex.new_value)) || '%'
  AND ex.new_value !~* '^(instrumental|version|remix|live|demo|acoustic|karaoke|pop|finale|reprise|single|edit|mono|stereo|extended|main title|end title|prologue|epilogue|score|medley|theme|intro|outro|interlude|prelude|overture|underture|bonus|deleted|alternate|original|soundtrack|film|movie|radio|short|long|full|complete|part|vol\.?|disc|cd|act|scene|clip|video|official|lyric|traduction|doublage|french|english|german|deutsch|espanol|cast|ensemble|orchestra|chorus|choeur|suite|symphonic|concert|deluxe|edition|remaster(ed)?|19\d\d|20\d\d)\b';

-- P2 -------------------------------------------------------------------------
WITH ex AS (
    SELECT ta.id AS answer_id,
           btrim(coalesce(
               -- "… from X" / "… de X" (quoted or bare, close-quote optional)
               substring(ta.value from $$(?i)(?:from|de|of|of the)\s+["“']?([^"“'()\[\]/]{2,60}?)\s*["“']?\s*$;?\s*$$),
               -- "X: Season N…" / "X - Vol. N" / "X, Vol. N" / "X Vol. N" / "X: Volume N"
               substring(ta.value from $$(?i)^(.{2,80}?)\s*(?::\s*(?:season|vol\.?|volume|episodes|chapters)\b|,?\s*-?\s*vol\.?\s*\d|\s+vol\.?\s*\d|\s+volume\s+\d)$$),
               -- "X - The Complete … Playlist" / "X - L'album…"
               substring(ta.value from $$^([^()\[\]:]{2,80}?)\s+-\s+(?:the complete|l'album|l’album|complete|original soundtrack album)$$),
               -- "X SONG COLLECTION" / "X Vocal Collection" / "X Sound Collection"
               substring(ta.value from $$(?i)^(.{2,80}?)\s+(?:vocal|song|sound|music|songs)[ &]*collection\b$$),
               -- leading parens strip: "La Princesse Et La Grenouille (The …)"
               substring(ta.value from $$^([^()\[\]:]{3,80}?)\s*[\(\[]$$)
           )) AS new_value
    FROM track_answers ta
    JOIN answer_types at ON at.id = ta.answer_type_id
    WHERE at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
      AND ta.value ~* '(classic|collection|songbook|princesses|piano|wedding|magie|animaux|ronde|legacy|chansons|songs|giraffenaffen|playhouse|villains|karaoke|sing|tribute|covers|orchestr|playlist|compilation|favorites|greatest|best of|official|resort|parks|ukulele|lullaby|mania|renditions|volume|vol\.|season|episodes|chapters|remix|version|album|gold|platinum|ultimate|anthology|essentials|complete|hits|popcorn|m[ée]lodies|monde|world|\(|\))'
)
UPDATE track_answers ta
SET value = ex.new_value, updated_at = now()
FROM ex
WHERE ta.id = ex.answer_id
  AND ex.new_value IS NOT NULL
  AND length(ex.new_value) >= 2
  AND ex.new_value !~* '^(the )?(disney|walt disney.*|disneyland|magic kingdom|epcot|chansons?|collection|hits|classics?|princesses?|popcorn|we love|weddings?|junior|animaux|piano|orchestra|season|volume|vol\.?|part|disc|soundtrack|official|songs?|story|medley|album|version|karaoke|playlist|compilation|favorites|greatest|best of|music|score|series|netflix.*|hbo.*|original.*)$'
  AND ex.new_value !~* '^(instrumental|version|remix|live|demo|acoustic|karaoke|single|edit|extended|score|medley|theme|intro|outro|prelude|overture|bonus|soundtrack|film|movie|complete|part|vol\.?|disc|official|suite|symphonic|edition|remaster(ed)?)\b'
  AND lower(btrim(ex.new_value)) <> lower(btrim(ta.value));

-- P3 -------------------------------------------------------------------------
-- Whitelisted junk values where the track TITLE is the work name
-- (cover/theme compilations list songs under their show's name).
UPDATE track_answers ta
SET value = ex.new_value, updated_at = now()
FROM (
    SELECT ta.id AS answer_id,
           btrim(regexp_replace(tt.value,
               $$(?i)\s+\((?:movie|film|main|title|theme|ost|soundtrack|opening|ending|générique)[^)]*\)$|\s*\(["“”][^)]*\)$|\s+theme$|\s+theme song$|\s+main theme$|\s+opening$|\s+ending$|\s+intro$|\s+générique$$,
               '')) AS new_value
    FROM track_answers ta
    JOIN answer_types at ON at.id = ta.answer_type_id
    JOIN track_answers tt ON tt.track_id = ta.track_id
    JOIN answer_types att ON att.id = tt.answer_type_id AND att.name = 'Title'
    WHERE at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
      AND ta.value IN (
        'Classic Cartoon Theme',
        'Les plus belles chansons de Dorothée',
        'Chansons pour les enfants',
        'Ultimate TV Theme Tunes',
        'The 200 Ultimate Movie',
        'Movie Classics',
        'Cartoon Covers',
        '40 Chansons D''or'
      )
) ex
WHERE ta.id = ex.answer_id
  AND length(ex.new_value) >= 2
  AND lower(btrim(ex.new_value)) <> lower(btrim(ta.value));

-- Anime Collection is entirely Naruto covers on this catalog
UPDATE track_answers ta
SET value = 'Naruto',
    aliases = coalesce(ta.aliases,'[]'::jsonb) || '"Naruto"'::jsonb,
    updated_at = now()
FROM answer_types at
WHERE at.id = ta.answer_type_id
  AND at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
  AND ta.value IN ('Anime Collection','Karaoke Version from Naruto','Rock Karaoke Version from Naruto','Piano Version from Naruto','Karaoke - From Naruto','Karaoke Rock Version from Naruto','From touhou Karaoke Version');

-- P4 -------------------------------------------------------------------------
CREATE TEMP TABLE _media_fix4(track_pattern citext, media_value citext, add_aliases jsonb);
INSERT INTO _media_fix4 VALUES
 ('The Castle in the Sky',            'Le Château dans le Ciel',   '["Castle in the Sky","Laputa","Laputa: Castle in the Sky"]'),
 ('The Wind rises',                   'Le Vent se Lève',           '["The Wind Rises","Kaze Tachinu"]'),
 ('Kingdom Hearts - Dearly Beloved',  'Kingdom Hearts',            '[]'),
 ('Interstellar Cornfield Chase',     'Interstellar',              '[]'),
 ('Also sprach Zarathustra%',         '2001: L''Odyssée de l''Espace', '["2001: A Space Odyssey","2001","2001 l''odyssée de l''espace"]'),
 ('The Night Of%',                    'The Night Of',              '[]'),
 ('Radical Dreamers',                 'Chrono Cross',              '[]'),
 ('Scars of Time',                    'Chrono Cross',              '[]'),
 ('The Cogs of Fate',                 'Chrono Trigger',            '[]'),
 ('Future Days',                      'The Last of Us',            '["The Last of Us Part II","The Last of Us 2"]'),
 ('Wayfaring Stranger',               'The Last of Us',            '["The Last of Us Part II","The Last of Us 2"]'),
 ('Skyrim Piano Medley',              'The Elder Scrolls V: Skyrim','["Skyrim","The Elder Scrolls V","Elder Scrolls Skyrim"]'),
 ('A Bell is Tolling',                'Paper Mario',               '["Paper Mario: The Thousand-Year Door"]'),
 ('Au Pays de Candy',                 'Candy',                     '["Candy Candy","Candy-Candy"]'),
 ('Candy s''endort',                  'Candy',                     '["Candy Candy","Candy-Candy"]'),
 ('Voici venir les Barbapapa',        'Barbapapa',                 '["Les Barbapapa"]'),
 ('Daredevil (Movie Main Theme)',     'Daredevil',                 '[]'),
 ('E.T. (Movie Main Theme)',          'E.T.',                      '["E.T. l''extra-terrestre","E.T. the Extra-Terrestrial","ET"]'),
 ('Hulk (Movie Main Theme)',          'Hulk',                      '["The Incredible Hulk"]'),
 ('Misirlou (Chanson issue de "Pulp Fiction")','Pulp Fiction',      '[]'),
 ('Princess Mononoke (Symphonic Suite%','Princesse Mononoké',      '["Princess Mononoke","Mononoke"]'),
 ('The Legend of Ashitaka%',          'Princesse Mononoké',        '["Princess Mononoke","Mononoke"]'),
 ('Hyrule Castle',                    'The Legend of Zelda: Breath of the Wild','["Breath of the Wild","Zelda Breath of the Wild","Zelda BOTW"]'),
 ('Parasail Minigame%',               'The Legend of Zelda: Breath of the Wild','["Breath of the Wild","Zelda Breath of the Wild","Zelda BOTW"]'),
 ('Rito Village',                     'The Legend of Zelda: Breath of the Wild','["Breath of the Wild","Zelda Breath of the Wild","Zelda BOTW"]'),
 ('Diver (Karaoke Version from Naruto)','Naruto',                  '[]'),
 ('Haruka Kanata%',                   'Naruto',                    '[]'),
 ('Michi - to You All%',              'Naruto',                    '[]'),
 ('Nakushita Kotoba%',                'Naruto',                    '[]'),
 ('Pareedo%',                         'Naruto',                    '[]'),
 ('Anger and pain',                   'Lady Oscar',                '["The Rose of Versailles","La Rose de Versailles","Versailles no Bara"]'),
 ('Believe in My Way',                'Lady Oscar',                '["The Rose of Versailles","La Rose de Versailles","Versailles no Bara"]'),
 ('Child of Mars',                    'Lady Oscar',                '["The Rose of Versailles","La Rose de Versailles","Versailles no Bara"]'),
 ('Ma Vie en Rose',                   'Lady Oscar',                '["The Rose of Versailles","La Rose de Versailles","Versailles no Bara"]'),
 ('Resonance of Love',                'Lady Oscar',                '["The Rose of Versailles","La Rose de Versailles","Versailles no Bara"]'),
 ('Return to nothing',                'Lady Oscar',                '["The Rose of Versailles","La Rose de Versailles","Versailles no Bara"]'),
 ('Soul to Soul',                     'Lady Oscar',                '["The Rose of Versailles","La Rose de Versailles","Versailles no Bara"]'),
 ('The Rose of Versailles',           'Lady Oscar',                '["The Rose of Versailles","La Rose de Versailles","Versailles no Bara"]'),
 ('夜をこめて',                       'Lady Oscar',                '["The Rose of Versailles","La Rose de Versailles","Versailles no Bara"]'),
 ('心の在り処',                       'Lady Oscar',                '["The Rose of Versailles","La Rose de Versailles","Versailles no Bara"]'),
 ('Annie',                            'Annie',                     '[]');

UPDATE track_answers ta
SET value = mf.media_value,
    aliases = CASE WHEN mf.add_aliases = '[]'::jsonb THEN ta.aliases
                   ELSE coalesce(ta.aliases, '[]'::jsonb) || mf.add_aliases END,
    updated_at = now()
FROM _media_fix4 mf, answer_types at, track_answers tt, answer_types att
WHERE at.id = ta.answer_type_id
  AND tt.track_id = ta.track_id
  AND att.id = tt.answer_type_id AND att.name = 'Title'
  AND tt.value ILIKE mf.track_pattern
  AND at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
  AND ta.value ~* '(classic|collection|songbook|princesses|piano|wedding|magie|animaux|ronde|legacy|chansons|songs|giraffenaffen|playhouse|villains|karaoke|sing|tribute|covers|orchestr|playlist|compilation|favorites|greatest|best of|official|resort|parks|ukulele|lullaby|mania|renditions|volume|vol\.|season|episodes|chapters|suite|remix|version|album|gold|platinum|ultimate|anthology|essentials|complete|hits|popcorn|m[ée]lodies|monde|world|geek|misirlou|moment)';

DROP TABLE _media_fix4;
