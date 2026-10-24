-- La page conditions-generales existait déjà en prod (données héritées) :
-- l'INSERT de 0031 est ignoré → on met à jour le contenu vers la version
-- complète (abonnements, règles de jeu, droit applicable).

UPDATE pages SET content = $c$
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
<p>Les présentes CGU sont soumises au droit français. Tout litige relève des juridictions compétentes françaises.</p>
$c$, updated_at = NOW()
WHERE slug = 'conditions-generales';
