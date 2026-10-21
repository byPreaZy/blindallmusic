-- Remove em-dashes (—) from admin-authored copy: CMS pages, FAQs and
-- official room descriptions. Replaced with the punctuation that fits the
-- sentence (: ; , or parentheses), then a generic sweep for leftovers.

-- CMS legal pages -------------------------------------------------------------

UPDATE pages SET content = replace(content,
    'Clère Kévin — Clere''ly Design</p>',
    'Clère Kévin (Clere''ly Design)</p>')
WHERE content LIKE '%Clère Kévin — Clere''ly Design</p>%';

UPDATE pages SET content = replace(content,
    'Clère Kévin — Clere''ly Design — <a',
    'Clère Kévin (Clere''ly Design) : <a')
WHERE content LIKE '%Clère Kévin — Clere''ly Design — <a%';

UPDATE pages SET content = replace(content,
    'Clère Kévin — Clere''ly Design',
    'Clère Kévin (Clere''ly Design)')
WHERE content LIKE '%Clère Kévin — Clere''ly Design%';

UPDATE pages SET content = replace(content,
    'IONOS SARL — 7 place de la Gare, 57200 Sarreguemines, France — <a',
    'IONOS SARL, 7 place de la Gare, 57200 Sarreguemines, France : <a')
WHERE content LIKE '%IONOS SARL — 7 place de la Gare%';

UPDATE pages SET content = replace(content,
    'sans garantie de disponibilité permanente — il est maintenu bénévolement',
    'sans garantie de disponibilité permanente ; le service est maintenu bénévolement')
WHERE content LIKE '%disponibilité permanente —%';

UPDATE pages SET content = replace(content,
    'contacte d''abord l''équipe via la page Contact — une solution amiable',
    'contacte d''abord l''équipe via la page Contact : une solution amiable')
WHERE content LIKE '%page Contact —%';

UPDATE pages SET content = replace(content,
    'en stockage local — pas de traceur publicitaire',
    'en stockage local, sans traceur publicitaire')
WHERE content LIKE '%stockage local —%';

UPDATE pages SET content = replace(content,
    'efface vos données personnelles — depuis les paramètres',
    'efface vos données personnelles, depuis les paramètres')
WHERE content LIKE '%données personnelles —%';

-- Fallback for any remaining em-dash inside page copy.
UPDATE pages SET content = replace(content, ' — ', ' : ')
WHERE content LIKE '% — %';

-- FAQs ------------------------------------------------------------------------

UPDATE faqs SET answer = replace(answer,
    'plus tu marques de points — le premier',
    'plus tu marques de points : le premier')
WHERE answer LIKE '%marques de points —%';

UPDATE faqs SET answer = replace(answer,
    'dans le champ prévu — artiste, titre',
    'dans le champ prévu : artiste, titre')
WHERE answer LIKE '%champ prévu —%';

UPDATE faqs SET answer = replace(answer,
    'Chaque source a un plafond — les détails',
    'Chaque source a un plafond ; les détails')
WHERE answer LIKE '%un plafond —%';

UPDATE faqs SET answer = replace(answer,
    'n''affectent pas l''ELO — parfait pour jouer tranquille',
    'n''affectent pas l''ELO, parfait pour jouer tranquille')
WHERE answer LIKE '%l''ELO —%';

UPDATE faqs SET answer = replace(answer,
    'Oui — le site est une PWA',
    'Oui, le site est une PWA')
WHERE answer LIKE '%Oui —%';

UPDATE faqs SET answer = replace(answer,
    'via son menu — les modérateurs',
    'via son menu ; les modérateurs')
WHERE answer LIKE '%son menu —%';

UPDATE faqs SET answer = replace(answer,
    'avec le même bouton — pas besoin de mot de passe',
    'avec le même bouton, pas besoin de mot de passe')
WHERE answer LIKE '%même bouton —%';

UPDATE faqs SET question = replace(question, ' — ', ' : ')
WHERE question LIKE '% — %';

UPDATE faqs SET answer = replace(answer, ' — ', ' : ')
WHERE answer LIKE '% — %';

-- Official room descriptions ("Room officielle X — blind test en ligne") ------

UPDATE rooms SET description = replace(description, ' — ', ', ')
WHERE description LIKE '% — %';
