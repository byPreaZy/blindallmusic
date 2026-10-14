-- 1) Types supplémentaires (ids après les 11 du seeder original).
INSERT INTO answer_types (id, name, pronoun, svg_icon, created_at, updated_at)
SELECT v.id, v.name, v.pronoun, v.svg_icon, v.created_at, v.updated_at
FROM (VALUES
    (12::bigint, 'Video game'::citext, 'the'::citext,
     '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" viewBox="0 0 16 16">
  <path d="M11.5 6.027A.5.5 0 0 1 12 6.5l-.867 5.483a1 1 0 0 1-1.996.084L8.5 9.5h-1l-.637 2.567a1 1 0 0 1-1.996-.084L4 6.5a.5.5 0 0 1 .5-.473 7.53 7.53 0 0 1 7 0zM4.5 8a.5.5 0 0 0-.5.5v1a.5.5 0 0 0 .5.5h.5a.5.5 0 0 0 .5-.5v-1a.5.5 0 0 0-.5-.5h-.5zm2-.5a.5.5 0 0 1 .5.5v.5h.5a.5.5 0 0 1 0 1H7v.5a.5.5 0 0 1-1 0V10h-.5a.5.5 0 0 1 0-1H6V8a.5.5 0 0 1 .5-.5zm4.5 0a.5.5 0 1 1 0 1 .5.5 0 0 1 0-1zm1 1.5a.5.5 0 1 1 0 1 .5.5 0 0 1 0-1z"/>
</svg>'::text, NOW()::timestamptz, NOW()::timestamptz),
    (13::bigint, 'Musical'::citext, 'the'::citext,
     '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" viewBox="0 0 16 16">
  <path d="M9 13c0 1.105-1.12 2-2.5 2S4 14.105 4 13s1.12-2 2.5-2 2.5.895 2.5 2z"/><path fill-rule="evenodd" d="M9 3v10H8V3h1z"/><path d="M8 2.82a1 1 0 0 1 .804-.98l3-.6A1 1 0 0 1 13 2.22V4L8 5V2.82z"/>
</svg>'::text, NOW()::timestamptz, NOW()::timestamptz),
    (14::bigint, 'Character'::citext, 'the'::citext,
     '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" viewBox="0 0 16 16">
  <path d="M15 14s1 0 1-1-1-4-5-4-5 3-5 4 1 1 1 1h8zm-7.978-1A.261.261 0 0 1 7 12.996c.001-.264.167-1.03.76-1.72C8.312 10.629 9.282 10 11 10c1.717 0 2.687.63 3.24 1.276.593.69.758 1.457.76 1.72l-.008.002a.274.274 0 0 1-.014.002H7.022zM11 7a2 2 0 1 0 0-4 2 2 0 0 0 0 4zm3-2a3 3 0 1 1-6 0 3 3 0 0 1 6 0zM6.936 9.28a5.88 5.88 0 0 0-1.23-.247A7.35 7.35 0 0 0 5 9c-4 0-5 3-5 4 0 .667.333 1 1 1h4.216A2.238 2.238 0 0 1 5 13c0-1.01.377-2.042 1.09-2.904.243-.294.526-.569.846-.816zM4.92 10A5.493 5.493 0 0 0 4 13H1c0-.26.164-1.03.76-1.724.545-.636 1.492-1.256 3.16-1.275zM1.5 5.5a3 3 0 1 1 6 0 3 3 0 0 1 0-4z"/>
</svg>'::text, NOW()::timestamptz, NOW()::timestamptz)
) AS v(id, name, pronoun, svg_icon, created_at, updated_at)
WHERE NOT EXISTS (SELECT 1 FROM answer_types a WHERE a.id = v.id);

SELECT setval(pg_get_serial_sequence('answer_types', 'id'),
              GREATEST((SELECT max(id) FROM answer_types), 1), true);

WITH parens AS (
    SELECT ta.track_id AS tid, m[1] AS ptext
    FROM track_answers ta
    CROSS JOIN LATERAL regexp_matches(ta.value::text, '\(([^()]*)\)', 'g') AS m
    WHERE ta.answer_type_id IN (1, 2)
),
feat AS (
    SELECT DISTINCT tid AS track_id,
        btrim(regexp_replace(ptext,
            '^.*?(featuring|feat\.?|ft\.?|duet with)\s+', '', 'i'),
            ' .,–—-') AS full_name,
        btrim(regexp_replace(
            btrim(regexp_replace(ptext,
                '^.*?(featuring|feat\.?|ft\.?|duet with)\s+', '', 'i')),
            '\s+(of|from)\s+.*$', '', 'i'), ' .,–—-') AS short_name
    FROM parens
    WHERE ptext ~* '(^|[\s,])(featuring|feat\.?|ft\.?|duet with)\s'
)
INSERT INTO track_answers (track_id, answer_type_id, value, aliases, score, created_at, updated_at)
SELECT track_id, 5, short_name,
    CASE WHEN full_name <> short_name AND length(full_name) >= 2
         THEN jsonb_build_array(full_name) ELSE NULL END,
    0.5, NOW(), NOW()
FROM feat f
WHERE length(short_name) >= 2
  AND short_name !~ '^\d+$'
  AND NOT EXISTS (
      SELECT 1 FROM track_answers x
      WHERE x.track_id = f.track_id AND x.answer_type_id = 5
        AND lower(btrim(x.value::text)) = lower(f.short_name));

-- 3) Réponse média — extrait les noms d'œuvres des parenthèses du titre.
WITH names AS (
    SELECT t.id AS tid, r.name::text AS n
    FROM tracks t
    JOIN playlist_room pr ON pr.playlist_id = t.playlist_id
    JOIN rooms r ON r.id = pr.room_id
    UNION ALL
    SELECT t.id, p.name::text FROM tracks t
    JOIN playlists p ON p.id = t.playlist_id
),
sig AS (
    SELECT DISTINCT ON (tid) tid AS track_id, type_id
    FROM (
        SELECT tid,
          CASE
            WHEN n ~* '\y(video.?games?|videogames?|gaming|gamers?|jeu|jeux)\y' THEN
                (SELECT id FROM answer_types
                 WHERE lower(name::text) IN ('video game','video games','videogame','jeu vidéo','jeux vidéo','game')
                 ORDER BY id LIMIT 1)
            WHEN n ~* '\y(anime|animes|manga|mangas|japan|japon|otaku|naruto)\y' THEN 8
            WHEN n ~* 'anim' THEN 9
            WHEN n ~* '\y(série|séries|serie|series|show|shows|tv|télé|tele|netflix|feuilleton)\y' THEN 7
            WHEN n ~* '\y(film|films|movie|movies|cinema|cinéma|cine|disney|pixar|dreamworks|marvel)\y' THEN 6
            ELSE NULL END AS type_id,
          CASE
            WHEN n ~* '\y(video.?games?|videogames?|gaming|gamers?|jeu|jeux)\y' THEN 1
            WHEN n ~* '\y(anime|animes|manga|mangas|japan|japon|otaku|naruto)\y' THEN 2
            WHEN n ~* 'anim' THEN 3
            WHEN n ~* '\y(série|séries|serie|series|show|shows|tv|télé|tele|netflix|feuilleton)\y' THEN 4
            WHEN n ~* '\y(film|films|movie|movies|cinema|cinéma|cine|disney|pixar|dreamworks|marvel)\y' THEN 5
            ELSE 9 END AS prio
        FROM names
    ) s
    WHERE type_id IS NOT NULL
    ORDER BY tid, prio
),
parens AS (
    SELECT ta.track_id AS tid, m[1] AS ptext
    FROM track_answers ta
    CROSS JOIN LATERAL regexp_matches(ta.value::text, '\(([^()]*)\)', 'g') AS m
    WHERE ta.answer_type_id = 2
),
media AS (
    SELECT DISTINCT ON (tid) tid AS track_id, raw_name
    FROM (
        SELECT tid,
          CASE
            -- Œuvre citée entre guillemets : « (De "Mulan") »,
            -- « (Theme From "World of Warcraft") », « (De « X ») »
            WHEN (ptext ~ '"[^"]+"' OR ptext ~ '«[^»]+»' OR ptext ~ '“[^”]+”')
                 AND ptext ~* '\y(from|de|tirée?|extrait|issu|bande|soundtrack|b\.?o\.?f?|ost|générique|thème|theme|feuilleton|série|serie|film|movie|jeu|jeux|score|opening|ending|version|mix|hymne)\y'
              THEN coalesce(substring(ptext from '"([^"]+)"'),
                            substring(ptext from '«([^»]+)»'),
                            substring(ptext from '“([^”]+)”'))
            -- « (From X) », « (De X) », « (Générique de X) » sans guillemets
            WHEN ptext ~* '^\s*(from|de|tirée?\s+de|extrait\s+de|extrait\s+d''|issu\s+de|theme\s+from|theme\s+of|thème\s+(de|du|de\s+la|d'')|générique\s+(de|du|de\s+la|d''|de\s+la\s+série)|bande\s+originale\s+(de|du|d''|de\s+la|du\s+film|du\s+jeu|de\s+la\s+série|de\s+la\s+saga)|b\.?o\.?f?\.?\s+(de|du|d''|de\s+la)|ost\s+(de|du|d''|de\s+la)|opening\s+(de|du|de\s+la)|ending\s+(de|du|de\s+la)|hymne\s+(de|du)|version\s+(du\s+film|du\s+jeu)|musique\s+(de|du))\s*'
              THEN btrim(regexp_replace(
                        regexp_replace(ptext,
                          '^\s*(from|de|tirée?\s+de|extrait\s+de|extrait\s+d''|issu\s+de|theme\s+from|theme\s+of|thème\s+(de|du|de\s+la|d'')|générique\s+(de|du|de\s+la|d''|de\s+la\s+série)|bande\s+originale\s+(de|du|d''|de\s+la|du\s+film|du\s+jeu|de\s+la\s+série|de\s+la\s+saga)|b\.?o\.?f?\.?\s+(de|du|d''|de\s+la)|ost\s+(de|du|d''|de\s+la)|opening\s+(de|du|de\s+la)|ending\s+(de|du|de\s+la)|hymne\s+(de|du)|version\s+(du\s+film|du\s+jeu)|musique\s+(de|du))\s*', '', 'i'),
                        '\s*[/\-–—]?\s*(soundtrack|bande\s+originale.*|b\.?o\.?f?\.?.*|o\.?s\.?t\.?|ost|score|du\s+film|du\s+jeu|de\s+la\s+série|de\s+la\s+saga|française?.*|version.*)\s*$', '', 'i'))
            -- « (X Soundtrack) », « (X Bande originale) », « (X OST) »
            WHEN ptext ~* '\y(soundtrack|bande\s+originale|b\.?o\.?f?\.?|o\.?s\.?t\.?|ost|score)\s*$'
              THEN btrim(regexp_replace(ptext,
                        '\s*[/\-–—]?\s*(original\s+motion\s+picture|motion\s+picture|original|official|the)?\s*(soundtrack|bande\s+originale.*|b\.?o\.?f?\.?|o\.?s\.?t\.?|ost|score)\s*$', '', 'i'))
            ELSE NULL END AS raw_name
        FROM parens
        WHERE ptext ~* '\y(from|de|tirée?|extrait|issu|bande|soundtrack|bof|b\.?o\.?|ost|générique|thème|theme|feuilleton|série|serie|film|movie|jeu|jeux|score|opening|ending|hymne)\y|"[^"]+"|«[^»]+»|“[^”]+”'
    ) e
    WHERE raw_name IS NOT NULL
    ORDER BY tid, length(raw_name)
),
cleaned AS (
    SELECT track_id,
        btrim(regexp_replace(
            regexp_replace(raw_name, '\s+', ' ', 'g'),
            '\s*[-–—]?\s*(original\s+(video)?games?|original\s+score|original)\s*$', '', 'i'),
            ' -–—,/''"«»“”') AS name
    FROM media
)
INSERT INTO track_answers (track_id, answer_type_id, value, aliases, score, created_at, updated_at)
SELECT f.track_id, coalesce(s.type_id, 6), f.name,
    CASE WHEN position(',' in f.name) > 1
         THEN jsonb_build_array(btrim(split_part(f.name, ',', 1)))
         ELSE NULL END,
    0.5, NOW(), NOW()
FROM cleaned f
LEFT JOIN sig s ON s.track_id = f.track_id
WHERE length(f.name) >= 2
  AND f.name !~ '^\d+(\.\d+)?$'
  AND f.name !~* '^(feuilleton|film|série|serie|tv|télévision|télé|anime|dessin animé|jeu vidéo|jeux vidéo|version|remix|remastered?|live|edit|mix|deluxe|single|bande originale|soundtrack|ost|score)\y$'
  AND NOT EXISTS (
      SELECT 1 FROM track_answers x
      WHERE x.track_id = f.track_id
        AND x.answer_type_id IN (
            SELECT id FROM answer_types
            WHERE lower(name::text) IN ('movie','show','anime','cartoon',
                'video game','video games','videogame','jeu vidéo','jeux vidéo',
                'game','musical','character','documentary','drama')));
