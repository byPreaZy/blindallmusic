-- The "Événements spéciaux" category was renamed from "Divers" but kept
-- the seed slug `divers`, while category_content.rs ships dedicated SEO
-- copy keyed on `les-speciales`. Align the slug so the rich landing copy
-- is actually served; spa_page 301s /blind-test-divers → /blind-test-les-speciales.
UPDATE categories SET slug = 'les-speciales', updated_at = now()
WHERE slug = 'divers' AND name = 'Événements spéciaux';
