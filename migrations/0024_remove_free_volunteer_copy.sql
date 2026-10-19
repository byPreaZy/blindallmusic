-- Remove "entirely free" / volunteer-run framing from seeded legal & FAQ
-- content so the copy no longer rules out paid offerings. Idempotent:
-- REPLACE on already-updated content is a no-op, and the FAQ update keys on
-- the question text rather than the row id.

UPDATE pages
SET content = REPLACE(content, 'multijoueur, gratuit, édité', 'multijoueur édité'),
    updated_at = now()
WHERE slug = 'conditions-generales'
  AND content LIKE '%multijoueur, gratuit, édité%';

UPDATE faqs
SET question = 'Pourquoi les dons ?',
    answer = 'Blind''All Music se joue directement dans le navigateur, sans compte obligatoire (les invités peuvent jouer). Les dons servent à couvrir les coûts d''hébergement et d''API : quand l''objectif mensuel est atteint, toute publicité est désactivée pour tout le monde jusqu''à la fin du mois. Les donateurs reçoivent un badge et des avantages cosmétiques.',
    updated_at = now()
WHERE locale = 'fr'
  AND question LIKE '%payant%dons%';
