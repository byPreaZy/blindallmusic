-- Adresse de l'établissement + litiges sans médiateur désigné
-- (choix éditeur: renvoi au contact direct ; la plateforme ODR européenne
-- a fermé le 20 juillet 2025 — règlement (UE) 2024/3228).
-- NB: désigner un médiateur est formellement obligatoire pour le B2C
-- (art. L612-1 C. conso) — voir issue GitHub #5 pour le risque résiduel.

UPDATE pages SET content = replace(
    replace(content,
        '<em>[Adresse postale à compléter]</em>',
        '55 impasse des Buguets, 71500 Ratte, France'),
    'vous pouvez saisir gratuitement le médiateur de la consommation compétent : <em>[médiateur de la consommation à désigner]</em>, ainsi que la plateforme européenne de règlement des litiges <a href="https://ec.europa.eu/consumers/odr" target="_blank" rel="external nofollow">ec.europa.eu/consumers/odr</a>.',
    'nous t''invitons à nous contacter pour trouver une solution amiable. À défaut, les juridictions françaises sont compétentes.'),
    updated_at = NOW()
WHERE slug = 'conditions-generales-vente';

UPDATE pages SET content = replace(content,
    'Article 7 — Litiges et médiation', 'Article 7 — Litiges'),
    updated_at = NOW()
WHERE slug = 'conditions-generales-vente';

UPDATE pages SET content = replace(
    replace(content,
        '<em>[Adresse postale à compléter]</em>',
        '55 impasse des Buguets, 71500 Ratte, France'),
    'En cas de litige : <em>[médiateur de la consommation à désigner]</em>.',
    'En cas de litige : <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>. À défaut de solution amiable, juridictions françaises compétentes.'),
    updated_at = NOW()
WHERE slug = 'mentions-legales';
