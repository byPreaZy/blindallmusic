-- Correctif privacy: l'envoi d'e-mails se fait via Gmail SMTP (Google),
-- pas Brevo. 0031 is already applied on some environments, so the
-- correction is idempotent and standalone.
UPDATE pages SET content = $c$
<h2>Responsable de traitement</h2>
<p>Blind'All Music — Clère Kévin, Clere'ly Design (<a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>).</p>
<h2>Données collectées</h2>
<p><strong>Compte</strong> : pseudonyme, adresse e-mail, mot de passe (haché). <strong>Connexions OAuth</strong> (Google, Discord…) : identifiant du fournisseur uniquement. <strong>Jeu</strong> : scores, XP, historique de parties. <strong>Technique</strong> : adresse IP (anti-abus, journaux), navigateur.</p>
<h2>Sous-traitants</h2>
<ul>
<li><strong>Stripe</strong> (Stripe Payments Europe, Dublin) : traitement des paiements — dons <strong>et abonnements</strong> (Blind'All Plus/Pro). Blind'All Music ne stocke aucune donnée bancaire ; Stripe traite vos données selon sa propre politique de confidentialité et les garanties UE-États-Unis (Data Privacy Framework).</li>
<li><strong>IONOS</strong> (Allemagne/France) : hébergement des serveurs et bases de données.</li>
<li><strong>Google / Gmail SMTP</strong> (Google Ireland Ltd) : relais des e-mails transactionnels (vérification, mot de passe, bienvenue) — votre adresse e-mail et le contenu du message transitent par les serveurs Google.</li>
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
