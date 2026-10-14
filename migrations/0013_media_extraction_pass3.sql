-- Pass 3 media-answer normalization (Disney playlist focus, applied globally).
-- 0010's substring() patterns were case-SENSITIVE, so (From "X") with a capital
-- F was selected by the ~* WHERE but never extracted — only 145 lowercase
-- markers were fixed. Its pass B also rejected "The X"/"Le X" answers because
-- of a prefix-based generic-name reject, which is why "The Lion King" never
-- came out of "Legacy Collection: The Lion King".
--
-- Pass A: extract the work name from a marker paren/bracket in the track title:
--   (From "X")  [From ''X'']  (De "X")  (BOF "X")  (aus "X")  (Extrait de "X")
--   (End Title / From "X")  (From Disney's "X" / English Version)  (From "X)
-- Pass B: for junk media values, fall back to a bare final paren in the title
--   when it is not a version/edition marker:  Someday (The Hunchback of …)
-- Pass C: derive the work from the media value itself:
--   "Disney Sing-Along: Mary Poppins", "Legacy Collection: The Lion King",
--   "Violetta - L'album de la saison 1", "La Princesse Et La Grenouille (…)"
-- Pass D: curated mapping for compilation tracks with no usable marker.
-- Pass E: Featuring cleanup — junk ensemble answers (X - Cast, Studio Chorus,
--   album names) and duplicate Featuring values on the same track.

-- ---------------------------------------------------------------------------
-- Junk-media classifier used by passes B and C (NOT by A: the title marker is
-- authoritative even over a plausible media value).
-- ---------------------------------------------------------------------------

-- Pass A ----------------------------------------------------------------------
WITH ex AS (
    SELECT DISTINCT ON (ta.id)
        ta.id AS answer_id,
        ta.value AS old_value,
        btrim(regexp_replace(
            regexp_replace(
                coalesce(
                    -- quoted work inside the marker: prefer a properly paired
                    -- "X" FIRST, else an unclosed quote at paren end ("Toy Story 3)
                    substring(m.m from '["“”]([^"“”]{2,80}?)\s*["“”]'),
                    substring(m.m from '["“”]([^"“”/]{2,80}?)\s*["“”]?\s*$'),
                    -- ''X'' quoting, tolerating inner single quotes (Disney's X)
                    substring(m.m from $$''(.{2,80}?)''$$),
                    -- bare work after the keyword: (From The Mickey Mouse Club)
                    substring(m.m from $$^\s*([^"“”'/()\[\];]{2,120})\s*$$)
                ),
                $$(?i)^\s*(?:disney'?s|walt disney records|walt disney)\s+$$, ''),
            $$(?i)^["“”'\s]+|["“”']+\s*$$, '')) AS new_value
    FROM track_answers ta
    JOIN answer_types at ON at.id = ta.answer_type_id
    JOIN track_answers tt ON tt.track_id = ta.track_id
    JOIN answer_types att ON att.id = tt.answer_type_id AND att.name = 'Title'
    CROSS JOIN LATERAL (
        SELECT substring(tt.value from $$(?i)[\(\[]\s*(?:de|d'|du|des|from|aus|von|extrait(?:e|s)? d[ue]|bof|bo(?: d[ue])?|bande originale|version|theme song from|theme from|main title|end title|titre|music from|songs? from|featured in|g[ée]n[ée]rique|hauptthema|thema|dessin anim[ée]|film|soundtrack|issu[es]? d[ue]|tir[ée][es]? d[ue]|de la bande originale|de la s[ée]rie|du film|du dessin anim[ée]|original(?:e)? d[ue])\s*[:]?\s*([^()\[\]]{2,160})$$) AS m
    ) m
    WHERE at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
      AND m.m IS NOT NULL
    ORDER BY ta.id, length(m.m) DESC
)
UPDATE track_answers ta
SET value = ex.new_value, updated_at = now()
FROM ex
WHERE ta.id = ex.answer_id
  AND length(ex.new_value) >= 2
  AND lower(btrim(ex.new_value)) <> lower(btrim(ta.value))
  AND lower(btrim(ta.value)) NOT LIKE '%' || lower(btrim(ex.new_value)) || '%'
  -- refuse version/edition markers leaking into a media answer
  AND ex.new_value !~* '^(instrumental|version|remix|live|demo|acoustic|karaoke|pop|finale|reprise|single|edit|mono|stereo|extended|main title|end title|prologue|epilogue|score|medley|theme|intro|outro|interlude|prelude|overture|underture|bonus|deleted|alternate|original|soundtrack|film|movie|radio|short|long|full|complete|part|vol\.?|disc|cd|act|scene|clip|video|official|lyric|traduction|doublage|french|english|german|deutsch|deutsche|deutscher|espanol|cast|ensemble|orchestra|chorus|choeur|swedish|spanish|italian|portuguese|japanese|korean|mandarin|arabic|dutch|polish|russian|suite|symphonic|philharmonic|concert|unplugged|deluxe|anniversary|edition|remaster(ed)?|19\d\d|20\d\d)\b';

-- Pass B ----------------------------------------------------------------------
-- Junk media + bare trailing paren in title → the paren is the work name.
WITH ex AS (
    SELECT ta.id AS answer_id,
           btrim(substring(tt.value from $$\(([^()\[\]/"',;?!]{2,80})\)\s*$$)) AS new_value
    FROM track_answers ta
    JOIN answer_types at ON at.id = ta.answer_type_id
    JOIN track_answers tt ON tt.track_id = ta.track_id
    JOIN answer_types att ON att.id = tt.answer_type_id AND att.name = 'Title'
    WHERE at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
      AND ta.value ~* '(disney|classics?|collection|songbook|princesses?|piano|wedding|magie|magic|animaux|ronde|legacy|chansons|songs|giraffenaffen|playhouse|junior|villains|hits|celebration|album|vol\.|karaoke|sing|tribute|covers|orchestr|playlist|compilation|favorites|greatest|best of|official|resort|parks|ukulele|lullaby|story)'
)
UPDATE track_answers ta
SET value = ex.new_value, updated_at = now()
FROM ex
WHERE ta.id = ex.answer_id
  AND ex.new_value IS NOT NULL
  AND length(ex.new_value) >= 3
  AND ex.new_value !~* '^(instrumental|version|remix|live|demo|acoustic|karaoke|pop|finale|reprise|single|edit|mono|stereo|extended|main title|end title|prologue|epilogue|score|medley|theme|intro|outro|interlude|prelude|overture|underture|bonus|deleted|alternate|original|soundtrack|film|movie|radio|short|long|full|complete|part|vol\.?|disc|cd|act|scene|clip|video|official|lyric|traduction|doublage|french|english|german|deutsch|espanol|cast|ensemble|orchestra|chorus|choeur|suite|symphonic|concert|deluxe|edition|remaster(ed)?|19\d\d|20\d\d)\b'
  AND lower(btrim(ex.new_value)) <> lower(btrim(ta.value));

-- Pass C ----------------------------------------------------------------------
-- Junk media that embeds the work: "… Collection: X", "Sing-Along: X",
-- "Violetta - L'album…", "La Princesse Et La Grenouille (The Princess…)".
WITH ex AS (
    SELECT ta.id AS answer_id,
           btrim(coalesce(
               substring(ta.value from ':\s*([^:()\[\]]{2,80})$'),
               substring(ta.value from $$^([^()\[\]:/]{2,60}?)\s+-\s+[Ll]'?album$$),
               substring(ta.value from '^([^()\[\]:]{3,80}?)\s*[\(\[]')
           )) AS new_value
    FROM track_answers ta
    JOIN answer_types at ON at.id = ta.answer_type_id
    WHERE at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
      AND ta.value ~* '(disney|classics?|collection|songbook|princesses?|piano|wedding|magie|magic|animaux|ronde|legacy|chansons|songs|giraffenaffen|playhouse|junior|villains|hits|celebration|album|vol\.|karaoke|sing|tribute|covers|orchestr|playlist|compilation|favorites|greatest|best of|official|resort|parks|ukulele|lullaby|story|\(|\))'
)
UPDATE track_answers ta
SET value = ex.new_value, updated_at = now()
FROM ex
WHERE ta.id = ex.answer_id
  AND ex.new_value IS NOT NULL
  AND length(ex.new_value) >= 2
  -- whole-value generic reject (anchored): refuses "Disneyland", "The Collection"…
  AND ex.new_value !~* '^(the )?(disney|walt disney.*|disneyland|magic kingdom|epcot|chansons?|collection|hits|classics?|princesses?|popcorn|we love|weddings?|junior|animaux|piano|orchestra|season|volume|vol\.?|part|disc|soundtrack|official|songs?|story|medley|album|version|karaoke|playlist|compilation|favorites|greatest|best of)$'
  AND lower(btrim(ex.new_value)) <> lower(btrim(ta.value));

-- Pass D ----------------------------------------------------------------------
-- Curated fixes: compilation tracks with no usable marker anywhere.
-- ---------------------------------------------------------------------------
CREATE TEMP TABLE _media_fix(track_pattern citext, media_value citext, add_aliases jsonb);
INSERT INTO _media_fix VALUES
 ('Du hast ''n Freund in mir',        'Toy Story',                    '["You''ve Got a Friend in Me","Toy Story - Il était une fois des jouets"]'),
 ('A Nottingham',                     'Robin des Bois',               '["Robin Hood","Robin des bois"]'),
 ('Aie confiance',                    'Le Livre de la Jungle',        '["The Jungle Book","Livre de la jungle"]'),
 ('Des gammes et des arpèges',        'Les Aristochats',              '["The Aristocats","Aristocats"]'),
 ('Hi-Diddle Di Di',                  'Pinocchio',                    '[]'),
 ('Je suis en retard',                'Alice au Pays des Merveilles', '["Alice in Wonderland","Alice au pays des merveilles","Alice in Wonderland 1951"]'),
 ('La chansons des siamois',          'La Belle et le Clochard',      '["Lady and the Tramp","Belle et le Clochard"]'),
 ('Le Roi de mauvais aloi',           'Robin des Bois',               '["Robin Hood"]'),
 ('Les petites couturières%',         'Cendrillon',                   '["Cinderella"]'),
 ('Mais pourquoi m''en faire%',       'Robin des Bois',               '["Robin Hood"]'),
 ('Mon tout-petit',                   'Dumbo',                        '[]'),
 ('Thomas O''Malley',                 'Les Aristochats',              '["The Aristocats","Aristocats"]'),
 ('Voir voler un éléphant',           'Dumbo',                        '[]'),
 ('Ce rêve bleu',                     'Aladdin',                      '[]'),
 ('Chem-chem Cheminée',               'Mary Poppins',                 '[]'),
 ('Chim Chim Cher-Ee',                'Mary Poppins',                 '[]'),
 ('Cruella De Ville',                 'Les 101 Dalmatiens',           '["101 Dalmatians","One Hundred and One Dalmatians","Les Cent et Un Dalmatiens"]'),
 ('Cruella De Vil',                   'Les 101 Dalmatiens',           '["101 Dalmatians","One Hundred and One Dalmatians","Les Cent et Un Dalmatiens"]'),
 ('Cruela De Vil%',                   'Les 101 Dalmatiens',           '["101 Dalmatians","One Hundred and One Dalmatians","Les Cent et Un Dalmatiens"]'),
 ('Embrasse-la',                      'La Petite Sirène',             '["The Little Mermaid","Petite Sirene"]'),
 ('Heigh-Ho!',                        'Blanche-Neige et les Sept Nains', '["Snow White","Snow White and the Seven Dwarfs","Blanche Neige"]'),
 ('Il en faut peu pour être heureux', 'Le Livre de la Jungle',        '["The Jungle Book","Livre de la jungle"]'),
 ('J''en Ai Rêvé',                    'Raiponce',                     '["Tangled"]'),
 ('Je Suis Ton Ami',                  'Toy Story',                    '["Toy Story - Il était une fois des jouets"]'),
 ('L'' air Du Vent',                  'Pocahontas',                   '["Pocahontas - Une Légende Indienne","Colors of the Wind"]'),
 ('Le Monde Qui Est Le Mien',         'Le Livre de la Jungle',        '["The Jungle Book","Livre de la jungle"]'),
 ('Les Aristochats',                  'Les Aristochats',              '["The Aristocats","Aristocats"]'),
 ('Partir là-bas',                    'La Petite Sirène',             '["The Little Mermaid","Petite Sirene"]'),
 ('Si Je Ne T''avais Pas',            'Monstres et Cie',              '["Monsters, Inc.","Monsters Inc","Monstres & Cie"]'),
 ('Sous l''océan',                    'La Petite Sirène',             '["The Little Mermaid","Petite Sirene"]'),
 ('Supercalifragilistic',             'Mary Poppins',                 '[]'),
 ('Tout le monde veut devenir un cat','Les Aristochats',              '["The Aristocats","Aristocats"]'),
 ('Tu t''envoles',                    'Peter Pan',                    '[]'),
 ('Joyeux non-anniversaire',          'Alice au Pays des Merveilles', '["Alice in Wonderland","Alice au pays des merveilles"]'),
 ('Quand Elle M''aimait',             'Toy Story 2',                  '[]'),
 ('Une étoile Est Née',               'Hercule',                      '["Hercules"]'),
 ('A Dream Is A Wish Your Heart Makes%','Cendrillon',                 '["Cinderella"]'),
 ('A Whole New World (Instrumental)', 'Aladdin',                      '[]'),
 ('Beauty and the Beast (Instrumental)','La Belle et la Bête',        '["Beauty and the Beast"]'),
 ('Bella Notte%',                     'La Belle et le Clochard',      '["Lady and the Tramp","Belle et le Clochard"]'),
 ('Endless Night%',                   'Le Roi Lion',                  '["The Lion King","Le Roi Lion 1","The Lion King Musical"]'),
 ('If I Never Knew You (Instrumental)','Pocahontas',                  '["Pocahontas - Une Légende Indienne"]'),
 ('Love is a Song%',                  'Bambi',                        '[]'),
 ('Part of Your World (Instrumental)','La Petite Sirène',             '["The Little Mermaid","Petite Sirene"]'),
 ('So This Is Love (Instrumental)',   'Cendrillon',                   '["Cinderella"]'),
 ('Some Day My Prince Will Come%',    'Blanche-Neige et les Sept Nains', '["Snow White","Snow White and the Seven Dwarfs","Blanche Neige"]'),
 ('Someday My Prince Will Come%',     'Blanche-Neige et les Sept Nains', '["Snow White","Snow White and the Seven Dwarfs","Blanche Neige"]'),
 ('Something There%',                 'La Belle et la Bête',          '["Beauty and the Beast"]'),
 ('When You Wish Upon a Star%',       'Pinocchio',                    '[]'),
 ('When you Wish Upon a Star%',       'Pinocchio',                    '[]'),
 ('Baby Mine',                        'Dumbo',                        '[]'),
 ('Mary Poppins Medley',              'Mary Poppins',                 '[]'),
 ('Reflection (Instrumental)',        'Mulan',                        '[]'),
 ('When I See an Elephant Fly%',      'Dumbo',                        '[]'),
 ('Zip A Dee Doo Dah',                'Mélodie du Sud',               '["Song of the South","Melodie du Sud"]'),
 ('All In the Golden Afternoon',      'Alice au Pays des Merveilles', '["Alice in Wonderland","Alice au pays des merveilles"]'),
 ('Darkwing Duck Theme',              'Darkwing Duck',                '["Myster Mask","Darkwing Duck"]'),
 ('Gummi Bears Theme',                'Les Gummi',                    '["Gummi Bears","Disney''s Adventures of the Gummi Bears","The Gummi Bears"]'),
 ('The Naked Mole Rap',               'Kim Possible',                 '[]'),
 ('The Three Caballeros',             'Les Trois Caballeros',         '["The Three Caballeros","Trois Caballeros"]'),
 ('Theme from Zorro',                 'Zorro',                        '[]'),
 ('Theme to Lizzie McGuire%',         'Lizzie McGuire',               '[]'),
 ('Whistle Stop',                     'Robin des Bois',               '["Robin Hood"]'),
 ('Who, What, Why, Where, When and How Day','Les Aventures de Winnie l''Ourson','["Winnie l''Ourson","Winnie the Pooh","The New Adventures of Winnie the Pooh"]'),
 ('Just Around the Riverbend',        'Pocahontas',                   '["Pocahontas - Une Légende Indienne"]'),
 ('Somewhere Out There%',             'An American Tail',             '["Fievel et le Nouveau Monde","Fievel","Un monde à part"]'),
 ('Aloha, E Komo Mai',                'Lilo & Stitch',                '["Lilo and Stitch","Lilo et Stitch","Lilo & Stitch: The Series"]'),
 ('Docteur La Peluche Générique',     'Docteur La Peluche',           '["Doc McStuffins","Docteur la Peluche"]'),
 ('Henry Câlimonstre Générique',      'Henry Câlimonstre',            '["Henry Hugglemonster","Henry Calimonstre"]'),
 ('Jake et les Pirates du Pays Imaginaire%','Jake et les Pirates du Pays Imaginaire','["Jake and the Never Land Pirates"]'),
 ('Princesse Sofia Générique',        'Princesse Sofia',              '["Sofia the First","Princesse Sofia"]'),
 ('La Maison de Mickey, le générique','La Maison de Mickey',          '["Mickey Mouse Clubhouse","La Maison de Mickey"]'),
 ('C''est merveilleux d''être un tigre','Les Aventures de Winnie l''Ourson','["Winnie l''Ourson","Winnie the Pooh"]'),
 ('Snow White And The Seven Dwarfs Symphonic Suite%','Blanche-Neige et les Sept Nains','["Snow White","Snow White and the Seven Dwarfs","Blanche Neige"]'),
 ('Ser Mejor',                        'Violetta',                     '[]'),
 ('Écoute ton cœur%',                 'Pocahontas',                   '["Pocahontas - Une Légende Indienne"]'),
 ('Au détour de la rivière',          'Pocahontas',                   '["Pocahontas - Une Légende Indienne"]'),
 ('Filles du Roi Triton',             'La Petite Sirène',             '["The Little Mermaid","Petite Sirene"]'),
 ('Premier Baiser D''amour',          'Blanche-Neige et les Sept Nains', '["Snow White","Snow White and the Seven Dwarfs","Blanche Neige"]'),
 ('Un sourire en chantant',           'La Belle au Bois Dormant',     '["Sleeping Beauty","Belle au bois dormant"]'),
 ('Bonjour l''amour',                 'Bambi',                        '[]'),
 ('Can You Feel the Love Tonight',    'Le Roi Lion',                  '["The Lion King","Le Roi Lion 1"]'),
 ('Etrange Bazar',                    'Toy Story',                    '[]'),
 ('Jamais Plus Je Ne Volerai',        'Toy Story',                    '[]'),
 ('Western Woody',                    'Toy Story',                    '[]'),
 ('Colors of the Wind',               'Pocahontas',                   '["Pocahontas - Une Légende Indienne"]'),
 ('If I Never New You',               'Pocahontas',                   '["Pocahontas - Une Légende Indienne"]'),
 ('Listen with your Heart',           'Pocahontas',                   '["Pocahontas - Une Légende Indienne"]'),
 ('Mine Mine Mine',                   'Pocahontas',                   '["Pocahontas - Une Légende Indienne"]'),
 ('Savages Part %',                   'Pocahontas',                   '["Pocahontas - Une Légende Indienne"]'),
 ('Steady as the Beating Drum%',      'Pocahontas',                   '["Pocahontas - Une Légende Indienne"]'),
 ('The Virginia Company%',            'Pocahontas',                   '["Pocahontas - Une Légende Indienne"]');

UPDATE track_answers ta
SET value = mf.media_value,
    aliases = CASE WHEN mf.add_aliases = '[]'::jsonb THEN ta.aliases
                   ELSE coalesce(ta.aliases, '[]'::jsonb) || mf.add_aliases END,
    updated_at = now()
FROM _media_fix mf, answer_types at, track_answers tt, answer_types att
WHERE at.id = ta.answer_type_id
  AND tt.track_id = ta.track_id
  AND att.id = tt.answer_type_id AND att.name = 'Title'
  AND tt.value ILIKE mf.track_pattern
  AND at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
  AND ta.value ~* '(disney|classics?|collection|songbook|princesses?|piano|wedding|magie|magic|animaux|ronde|legacy|chansons|songs|giraffenaffen|playhouse|junior|villains|hits|celebration|album|vol\.|karaoke|sing|tribute|covers|orchestr|playlist|compilation|favorites|greatest|best of|official|resort|parks|ukulele|lullaby|story|^fall on me)';

DROP TABLE _media_fix;

-- Pass D2 --------------------------------------------------------------------
-- Value-level leftovers that the title-marker passes cannot reach: mangled
-- extractions from earlier passes and compilation names embedding the work.
UPDATE track_answers ta SET value='Frozen', updated_at=now()
FROM answer_types at
WHERE at.id=ta.answer_type_id AND at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
  AND ta.value ~* '^Frozen /';

UPDATE track_answers ta SET value='Toy Story',
    aliases=coalesce(ta.aliases,'[]'::jsonb)||'"Toy Story Music Mania"'::jsonb, updated_at=now()
FROM answer_types at
WHERE at.id=ta.answer_type_id AND at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
  AND ta.value ILIKE 'Toy Story Music Mania%';

UPDATE track_answers ta SET value='Pocahontas',
    aliases=coalesce(ta.aliases,'[]'::jsonb)||'"Pocahontas - Une Légende Indienne"'::jsonb, updated_at=now()
FROM answer_types at
WHERE at.id=ta.answer_type_id AND at.name IN ('Movie','Cartoon','Anime','Show','Video game','Musical')
  AND ta.value ~* 'Renditions of Pocahontas';

-- Pass D3 --------------------------------------------------------------------
-- Earlier passes stripped a leading "The" from canonical names; restore it and
-- keep the short form as an alias so both validate.
UPDATE track_answers SET
  value = 'The ' || value,
  aliases = coalesce(aliases, '[]'::jsonb) || to_jsonb(ARRAY[value]),
  updated_at = now()
WHERE value IN ('Lion King','Little Mermaid','Jungle Book','Princess and the Frog','Nutcracker And The Four Realms','Fox and the Hound','Hunchback of Notre Dame','Little Mermaid 2: Return to the Sea','Muppet Movie','Tigger Movie','Parent Trap','Happiest Millionaire','Haunted Mansion','Aristocats','Rescuers','American Tail','Little Mermaid 2','Lion King 2','Monkey''s Uncle','Mickey Mouse Club','Mickey Mouse Club TV Show','Carousel of Progress','Country Bear Vacation Hoedown','Country Bear Jamboree Show','Land','American Adventure','Nightmare Before Christmas','Rescuers Down Under','Three Caballeros','Sword in the Stone','Jungle Book 2','Lion King 1½','Many Adventures of Winnie the Pooh','Incredibles','Emperor''s New Groove','Princess Diaries','Little Mermaid: Ariel''s Beginning','Lion Guard','Good Dinosaur','Black Cauldron','Great Mouse Detective','Oliver & Company','Little Einsteins','Lion King II: Simba''s Pride','Little Mermaid II: Return to the Sea');

-- Pass E ----------------------------------------------------------------------
-- Featuring cleanup.
-- E1: junk ensemble answers (ensemble casts, album names, generic choruses).
DELETE FROM track_answers ta
USING answer_types at
WHERE at.id = ta.answer_type_id AND at.name = 'Featuring'
  AND ta.value ~* $$([-–—]?\s*cast[[:>:]]|cast\s*$|^cast[[:<:]]|giraffenaffen|studio chorus|disney studio|children's chorus|^chorus[[:>:]]|chorus\s*$|orchestre|orchestra\b.*disney|^disney$|^disney's|^the disney|ensemble vocal|choeurs?\b.*disney)$$;

-- E2: duplicate Featuring values on the same track (FR/DE/EN cast merges
-- produced rows like Joseph Williams ×2 on Hakuna Matata).
DELETE FROM track_answers a
USING track_answers b
WHERE a.answer_type_id = b.answer_type_id
  AND a.track_id = b.track_id
  AND a.answer_type_id = (SELECT id FROM answer_types WHERE name = 'Featuring')
  AND lower(btrim(a.value)) = lower(btrim(b.value))
  AND a.id > b.id;
