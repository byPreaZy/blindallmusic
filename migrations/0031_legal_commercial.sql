-- Legal & commercial compliance pages (France / EU).
-- CGV (Terms of Sale) + CGU (Terms of Use) seeded; legal notice and privacy
-- policy updated for paid subscriptions.
--
-- ⚠ BEFORE PRODUCTION LAUNCH the operator must fill in:
--   - SIREN / legal form / share capital in mentions-legales + CGV
--   - the chosen consumer mediator (e.g. CM2C, Médiation de la consommation
--     DEVIGNY, FEVAD…) in mentions-legales + CGV
--   - VAT status (TVA non applicable / n° TVA intracommunautaire)
-- Placeholder wording below is intentionally explicit about this.

-- ============================================================
-- CGV — Conditions Générales de Vente (subscriptions)
-- ============================================================
INSERT INTO pages (title, slug, content, created_at, updated_at)
SELECT 'Conditions Générales de Vente', 'conditions-generales-vente', $c$
<h2>Article 1 — Objet</h2>
<p>Les présentes Conditions Générales de Vente (CGV) régissent la vente des abonnements payants proposés sur Blind'All Music : <strong>Blind'All Plus</strong> (4,99 € TTC/mois) et <strong>Blind'All Pro</strong> (29,00 € TTC/mois). Toute souscription implique l'acceptation pleine et entière des présentes CGV et des <a href="/pages/conditions-generales">Conditions Générales d'Utilisation</a>.</p>
<h2>Article 2 — Éditeur</h2>
<p>Blind'All Music est édité par Clère Kévin — Clere'ly Design. Contact : <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>. <em>[SIREN et forme juridique à compléter par l'éditeur]</em></p>
<h2>Article 3 — Caractéristiques des offres</h2>
<p><strong>Blind'All Plus — 4,99 € TTC/mois</strong> : accès sans publicité, un « revive » quotidien sur le daily challenge, tag personnalisé, et l'ensemble des avantages supporter (couronne d'avatar, réactions).</p>
<p><strong>Blind'All Pro — 29,00 € TTC/mois</strong> : offre professionnelle pour bars et événements : affichage du nom de l'établissement sur l'écran TV des salles et photo de salle, plus les avantages supporter.</p>
<p>Les avantages sont attachés au compte souscrit et ne sont pas cessibles.</p>
<h2>Article 4 — Prix et facturation</h2>
<p>Les prix sont indiqués en euros, toutes taxes comprises (TTC). La facturation est mensuelle, à date anniversaire de la souscription, sans engagement de durée. Le paiement est traité par notre prestataire Stripe (Stripe Payments Europe, Dublin, Irlande) ; Blind'All Music ne stocke aucune donnée bancaire. Une facture/reçu électronique est envoyée par e-mail à chaque prélèvement.</p>
<h2>Article 5 — Renouvellement et résiliation</h2>
<p>L'abonnement se renouvelle automatiquement chaque mois. Vous pouvez résilier à tout moment, en quelques clics, depuis la rubrique « Paramètres » de votre compte, ou par e-mail à <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>. La résiliation prend effet à la fin de la période en cours : les avantages restent actifs jusqu'à cette date, conformément à l'article L215-1-1 du Code de la consommation (résiliation en ligne en 3 clics maximum).</p>
<h2>Article 6 — Droit de rétractation</h2>
<p>Conformément à l'article L221-18 du Code de la consommation, vous disposez en principe d'un délai de 14 jours pour exercer votre droit de rétractation. Toutefois, pour la fourniture de contenu numérique accessible immédiatement, <strong>en cochant la case de souscription vous demandez expressément l'exécution immédiate du service et renoncez expressément à votre droit de rétractation</strong> (article L221-28, 13° du Code de la consommation).</p>
<h2>Article 7 — Litiges et médiation</h2>
<p>En cas de litige, contactez d'abord <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>. À défaut d'accord amiable, vous pouvez saisir gratuitement le médiateur de la consommation compétent : <em>[médiateur à désigner par l'éditeur]</em>, ainsi que la plateforme européenne de règlement des litiges <a href="https://ec.europa.eu/consumers/odr" target="_blank" rel="external nofollow">ec.europa.eu/consumers/odr</a>.</p>
$c$, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM pages WHERE slug = 'conditions-generales-vente');

UPDATE pages SET content = $c$
<h2>Article 1 — Objet</h2>
<p>Les présentes Conditions Générales de Vente (CGV) régissent la vente des abonnements payants proposés sur Blind'All Music : <strong>Blind'All Plus</strong> (4,99 € TTC/mois) et <strong>Blind'All Pro</strong> (29,00 € TTC/mois). Toute souscription implique l'acceptation pleine et entière des présentes CGV et des <a href="/pages/conditions-generales">Conditions Générales d'Utilisation</a>.</p>
<h2>Article 2 — Éditeur</h2>
<p>Blind'All Music est édité par Clère Kévin — Clere'ly Design. Contact : <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>. <em>[SIREN et forme juridique à compléter par l'éditeur]</em></p>
<h2>Article 3 — Caractéristiques des offres</h2>
<p><strong>Blind'All Plus — 4,99 € TTC/mois</strong> : accès sans publicité, un « revive » quotidien sur le daily challenge, tag personnalisé, et l'ensemble des avantages supporter (couronne d'avatar, réactions).</p>
<p><strong>Blind'All Pro — 29,00 € TTC/mois</strong> : offre professionnelle pour bars et événements : affichage du nom de l'établissement sur l'écran TV des salles et photo de salle, plus les avantages supporter.</p>
<p>Les avantages sont attachés au compte souscrit et ne sont pas cessibles.</p>
<h2>Article 4 — Prix et facturation</h2>
<p>Les prix sont indiqués en euros, toutes taxes comprises (TTC). La facturation est mensuelle, à date anniversaire de la souscription, sans engagement de durée. Le paiement est traité par notre prestataire Stripe (Stripe Payments Europe, Dublin, Irlande) ; Blind'All Music ne stocke aucune donnée bancaire. Une facture/reçu électronique est envoyée par e-mail à chaque prélèvement.</p>
<h2>Article 5 — Renouvellement et résiliation</h2>
<p>L'abonnement se renouvelle automatiquement chaque mois. Vous pouvez résilier à tout moment, en quelques clics, depuis la rubrique « Paramètres » de votre compte, ou par e-mail à <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>. La résiliation prend effet à la fin de la période en cours : les avantages restent actifs jusqu'à cette date, conformément à l'article L215-1-1 du Code de la consommation (résiliation en ligne en 3 clics maximum).</p>
<h2>Article 6 — Droit de rétractation</h2>
<p>Conformément à l'article L221-18 du Code de la consommation, vous disposez en principe d'un délai de 14 jours pour exercer votre droit de rétractation. Toutefois, pour la fourniture de contenu numérique accessible immédiatement, <strong>en cochant la case de souscription vous demandez expressément l'exécution immédiate du service et renoncez expressément à votre droit de rétractation</strong> (article L221-28, 13° du Code de la consommation).</p>
<h2>Article 7 — Litiges et médiation</h2>
<p>En cas de litige, contactez d'abord <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>. À défaut d'accord amiable, vous pouvez saisir gratuitement le médiateur de la consommation compétent : <em>[médiateur à désigner par l'éditeur]</em>, ainsi que la plateforme européenne de règlement des litiges <a href="https://ec.europa.eu/consumers/odr" target="_blank" rel="external nofollow">ec.europa.eu/consumers/odr</a>.</p>
$c$, updated_at = NOW()
WHERE slug = 'conditions-generales-vente';

-- ============================================================
-- CGU — Conditions Générales d'Utilisation (was missing!)
-- ============================================================
INSERT INTO pages (title, slug, content, created_at, updated_at)
SELECT 'Conditions Générales d''Utilisation', 'conditions-generales', $c$
<h2>Article 1 — Objet</h2>
<p>Blind'All Music est un blind test musical en ligne : quiz multijoueurs en temps réel, mini-jeux quotidiens et classements. Les présentes CGU régissent l'accès et l'utilisation du service. Toute utilisation implique leur acceptation.</p>
<h2>Article 2 — Accès au service</h2>
<p>Le jeu est accessible gratuitement. Trois niveaux d'accès : <strong>invité</strong> (jouer dans les salons sans compte), <strong>membre</strong> (compte gratuit : mini-jeux, classements, chat, historique), <strong>abonné</strong> (Blind'All Plus ou Pro : avantages payants régis par les <a href="/pages/conditions-generales-vente">CGV</a>). Vous devez avoir 13 ans ou plus pour créer un compte.</p>
<h2>Article 3 — Compte</h2>
<p>Vous êtes responsable de la confidentialité de vos identifiants. Un compte est personnel et non cessible. Nous pouvons suspendre un compte en cas de triche, d'abus ou de comportement nuisible.</p>
<h2>Article 4 — Règles du jeu</h2>
<p>Interdits : bots/scripts de réponse automatique, exploitation de bugs, contournement des restrictions (multi-comptes abusifs), harcèlement dans le chat. Les scores et classements sont calculés par nos serveurs et peuvent être corrigés en cas de triche avérée.</p>
<h2>Article 5 — Contenus</h2>
<p>Les extraits audio sont des previews de 30 secondes fournis par les API publiques des plateformes (Deezer, Spotify, Apple Music…) ou des contenus sous licence libre uploadés par des modérateurs habilités. Vous ne pouvez pas réutiliser ces extraits hors du service.</p>
<h2>Article 6 — Responsabilité</h2>
<p>Le service est fourni « en l'état ». Nous nous efforçons d'assurer sa disponibilité mais ne garantissons pas l'absence d'interruption. Notre responsabilité est limitée aux dommages directs dans les conditions prévues par la loi.</p>
<h2>Article 7 — Droit applicable</h2>
<p>Les présentes CGU sont soumises au droit français. Tout litige relève des juridictions compétentes françaises, sans préjudice des règles de médiation prévues aux CGV.</p>
$c$, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM pages WHERE slug = 'conditions-generales');

-- ============================================================
-- Mentions légales — add commercial/mediator placeholders
-- ============================================================
UPDATE pages SET content = $c$
<h2>Éditeur du site</h2>
<p>Blind'All Music est édité par Clère Kévin, Clere'ly Design. <em>[SIREN / forme juridique à compléter]</em></p>
<p>Contact : <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a> ou via la page <a href="/contact">Contact</a>.</p>
<h2>Directeur de la publication</h2>
<p>Clère Kévin.</p>
<h2>Hébergement</h2>
<p>IONOS SARL : 7 place de la Gare, 57200 Sarreguemines, France, <a href="https://www.ionos.fr" target="_blank" rel="external nofollow">ionos.fr</a>.</p>
<h2>Vente en ligne</h2>
<p>Les abonnements payants (Blind'All Plus, Blind'All Pro) sont régis par les <a href="/pages/conditions-generales-vente">Conditions Générales de Vente</a>. Paiement sécurisé par Stripe. En cas de litige : <em>[médiateur de la consommation à désigner]</em>.</p>
<h2>Propriété intellectuelle</h2>
<p>Les extraits audio joués dans les quiz sont des previews de 30 secondes fournis par les API publiques des plateformes (Deezer, Spotify) ou des extraits sous licence libre uploadés par les modérateurs. Les marques et pochettes appartiennent à leurs ayants droit.</p>
$c$, updated_at = NOW()
WHERE slug = 'mentions-legales';

-- ============================================================
-- Privacy — Stripe for payments (not only donations) + retention
-- ============================================================
UPDATE pages SET content = $c$
<h2>Responsable de traitement</h2>
<p>Blind'All Music — Clère Kévin, Clere'ly Design (<a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>).</p>
<h2>Données collectées</h2>
<p><strong>Compte</strong> : pseudonyme, adresse e-mail, mot de passe (haché). <strong>Connexions OAuth</strong> (Google, Discord…) : identifiant du fournisseur uniquement. <strong>Jeu</strong> : scores, XP, historique de parties. <strong>Technique</strong> : adresse IP (anti-abus, journaux), navigateur.</p>
<h2>Sous-traitants</h2>
<ul>
<li><strong>Stripe</strong> (Stripe Payments Europe, Dublin) : traitement des paiements — dons <strong>et abonnements</strong> (Blind'All Plus/Pro). Blind'All Music ne stocke aucune donnée bancaire ; Stripe traite vos données selon sa propre politique de confidentialité et les garanties UE-États-Unis (Data Privacy Framework).</li>
<li><strong>IONOS</strong> (Allemagne/France) : hébergement des serveurs et bases de données.</li>
<li><strong>Google / Gmail SMTP</strong> (Irlande/UE via Google Ireland Ltd) : relais des e-mails transactionnels (vérification, mot de passe, bienvenue) — votre adresse e-mail et le contenu du message transitent par les serveurs Google.</li>
<li><strong>Google AdSense</strong> (via Google Funding Choices) : affichage publicitaire — uniquement avec votre consentement, et jamais aux supporters/abonnés.</li>
</ul>
<h2>Durées de conservation</h2>
<ul>
<li>Compte actif : jusqu'à suppression de compte (droit à l'effacement).</li>
<li>Journaux techniques : 12 mois maximum.</li>
<li>Données comptables liées aux paiements (factures Stripe) : 10 ans (obligations comptables, art. L123-22 Code de commerce).</li>
<li>Invités : données de jeu conservées puis rattachées au compte en cas d'inscription.</li>
</ul>
<h2>Cookies</h2>
<p>Cookies techniques indispensables (session, préférences) — pas de consentement requis. Cookies publicitaires AdSense — consentement préalable via la bannière (Funding Choices).</p>
<h2>Vos droits (RGPD)</h2>
<p>Accès, rectification, effacement, portabilité, opposition : écrivez à <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a> ou utilisez « Supprimer mon compte » dans les paramètres. Réclamation : CNIL, <a href="https://www.cnil.fr" target="_blank" rel="external nofollow">cnil.fr</a>.</p>
$c$, updated_at = NOW()
WHERE slug = 'politique-de-confidentialite';

-- ============================================================
-- FAQ — subscriptions + fix obsolete "ads off for everyone" claim
-- ============================================================
UPDATE faqs SET answer = 'Les supporters (don du mois) et les abonnés Blind''All Plus/Pro naviguent sans aucune publicité. Si l''objectif de dons du mois est atteint, le site reste financé — mais la navigation sans pub reste un avantage personnel des supporters et abonnés.'
WHERE question ILIKE '%pub%' OR question ILIKE '%ad%';

INSERT INTO faqs (locale, question, answer, created_at)
SELECT 'fr', 'C''est quoi Blind''All Plus ?',
'Blind''All Plus (4,99 €/mois TTC, sans engagement) : zéro publicité, un revive quotidien sur le daily challenge, tag personnalisé, et tous les avantages supporter (couronne, réactions). Résiliable à tout moment depuis vos paramètres.', NOW()
WHERE NOT EXISTS (SELECT 1 FROM faqs WHERE question ILIKE '%Blind''All Plus%');

INSERT INTO faqs (locale, question, answer, created_at)
SELECT 'fr', 'C''est quoi Blind''All Pro ?',
'Blind''All Pro (29 €/mois TTC) est pensé pour les bars et événements : affichez le nom de votre établissement sur l''écran TV de vos salles et une photo de salle personnalisée, en plus des avantages supporter.', NOW()
WHERE NOT EXISTS (SELECT 1 FROM faqs WHERE question ILIKE '%Blind''All Pro%');

INSERT INTO faqs (locale, question, answer, created_at)
SELECT 'fr', 'Comment résilier mon abonnement ?',
'En quelques clics depuis « Paramètres » de votre compte, ou par e-mail à clerelydesign@gmail.com. L''abonnement reste actif jusqu''à la fin de la période déjà payée.', NOW()
WHERE NOT EXISTS (SELECT 1 FROM faqs WHERE question ILIKE '%r%silier%');
