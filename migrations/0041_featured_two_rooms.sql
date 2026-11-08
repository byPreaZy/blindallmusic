-- Keep exactly two featured rooms: the home sidebar "Featured" section
-- has room for two cards, more overflowed the layout. featured_rooms()
-- also enforces LIMIT 2 so the section can never grow past that again.
UPDATE rooms SET is_featured = false, updated_at = now()
WHERE is_featured = true
  AND slug NOT IN ('chanson-francaise', 'quiz-general');
UPDATE rooms SET is_featured = true, updated_at = now()
WHERE slug IN ('chanson-francaise', 'quiz-general')
  AND is_featured = false;
