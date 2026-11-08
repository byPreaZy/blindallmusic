-- Product polish pass:
-- 1) Feature flagship rooms (home sidebar "Featured" was always empty).
-- 2) Rename "Nouvel an 2026" → "Nouvel an": the year in the name goes
--    stale; the room is seasonal ({12,1}) and returns every year.
-- 3) Fix ugly slugs (double-s typo, -0 suffix, dense slugification).
--    spa_page 301s the old URLs to the new slugs.

UPDATE rooms SET is_featured = true, updated_at = now()
WHERE slug IN ('chanson-francaise', 'rap-fr', 'disney', 'annees-80',
               'quiz-general', 'electro');

UPDATE rooms SET name = 'Nouvel an', slug = 'nouvel-an', updated_at = now()
WHERE slug = 'nouvel-an-2026';
UPDATE playlists SET name = 'Nouvel an', updated_at = now()
WHERE name = 'Nouvel an 2026';

UPDATE rooms SET slug = 'old-songs', updated_at = now() WHERE slug = 'old-songss';
UPDATE rooms SET slug = 'dessin-anime', updated_at = now() WHERE slug = 'dessin-anime-0';
UPDATE rooms SET slug = 'films-animations', updated_at = now() WHERE slug = 'films-danimations';
