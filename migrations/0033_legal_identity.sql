-- Identité légale réelle: Clere'ly Design, entreprise individuelle
-- (micro-entreprise), SIREN 942 291 634, TVA non applicable art. 293 B CGI.
-- Reste à compléter par l'éditeur : adresse postale + médiateur.

UPDATE pages SET content = replace(
    replace(content,
        '<em>[SIREN et forme juridique à compléter par l''éditeur]</em>',
        'Entreprise individuelle (micro-entreprise) — SIREN : 942 291 634. <em>[Adresse postale à compléter]</em>. TVA non applicable, art. 293 B du CGI.'),
    '<em>[médiateur à désigner par l''éditeur]</em>',
    '<em>[médiateur de la consommation à désigner]</em>'),
    updated_at = NOW()
WHERE slug = 'conditions-generales-vente';

UPDATE pages SET content = replace(content,
    '<em>[SIREN / forme juridique à compléter]</em>',
    'Entreprise individuelle (micro-entreprise) — SIREN : 942 291 634. <em>[Adresse postale à compléter]</em>. TVA non applicable, art. 293 B du CGI.'),
    updated_at = NOW()
WHERE slug = 'mentions-legales';
