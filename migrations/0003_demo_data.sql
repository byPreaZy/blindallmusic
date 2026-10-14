-- Utilisateur propriétaire des objets démo (password NULL → ne peut pas se
-- connecter, ce n'est pas un compte de test).
INSERT INTO users (name, email, elo, is_guest, show_donation_history_on_profile, created_at, updated_at)
SELECT 'demo', 'demo@blindall.test', 1500, false, true, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM users WHERE email = 'demo@blindall.test');

-- Admin de test E2E (password123) — la suite crates/blindall-e2e s'authentifie
-- dessus pour couvrir les surfaces admin/modération. Dev only.
INSERT INTO users (name, email, password, elo, is_administrator, is_guest,
                   email_verified_at, created_at, updated_at)
SELECT 'E2EAdmin', 'e2e-admin@blindall.test',
       '$2b$12$p1LGv2kQeq/NUZkLrYgB/.vNoXVF9gPp9wIWTKExFLBInCWV5kz.S',
       1500, true, false, NOW(), NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM users WHERE email = 'e2e-admin@blindall.test');

INSERT INTO playlists (name, user_id, is_public, description, created_at, updated_at)
SELECT 'Démo — Hits', u.id, true, 'Playlist de démonstration locale (previews SoundHelix).', NOW(), NOW()
FROM users u WHERE u.email = 'demo@blindall.test'
  AND NOT EXISTS (SELECT 1 FROM playlists WHERE name = 'Démo — Hits');

INSERT INTO rooms (user_id, name, description, category_id, slug, tracks_by_round,
                   track_duration, pause_between_tracks, pause_between_rounds,
                   is_public, is_random, is_chat_active, is_autostart, is_featured,
                   created_at, updated_at)
SELECT u.id, 'Démo', 'Room de démonstration — rejoignez et les rounds démarrent seuls.',
       1, 'demo', 5, 30, 5, 10, true, true, true, true, false, NOW(), NOW()
FROM users u WHERE u.email = 'demo@blindall.test'
  AND NOT EXISTS (SELECT 1 FROM rooms WHERE slug = 'demo');

INSERT INTO playlist_room (playlist_id, room_id, created_at, updated_at)
SELECT p.id, r.id, NOW(), NOW()
FROM playlists p, rooms r
WHERE p.name = 'Démo — Hits' AND r.slug = 'demo'
  AND NOT EXISTS (SELECT 1 FROM playlist_room pr WHERE pr.playlist_id = p.id AND pr.room_id = r.id);

-- Tracks — previews mp3 publiques (soundhelix.com, stables depuis des années).
INSERT INTO tracks (user_id, playlist_id, provider, provider_id, provider_url,
                    preview_url, artwork_url, created_at, updated_at)
SELECT u.id, p.id, 'demo', CONCAT('demo-', t.n), NULL, t.url, t.art, NOW(), NOW()
FROM users u
JOIN playlists p ON p.name = 'Démo — Hits'
JOIN (
    SELECT 1 AS n, 'https://www.soundhelix.com/examples/mp3/SoundHelix-Song-1.mp3' AS url, 'https://picsum.photos/seed/demo1/300/300' AS art
    UNION ALL SELECT 2, 'https://www.soundhelix.com/examples/mp3/SoundHelix-Song-2.mp3', 'https://picsum.photos/seed/demo2/300/300'
    UNION ALL SELECT 3, 'https://www.soundhelix.com/examples/mp3/SoundHelix-Song-3.mp3', 'https://picsum.photos/seed/demo3/300/300'
    UNION ALL SELECT 4, 'https://www.soundhelix.com/examples/mp3/SoundHelix-Song-4.mp3', 'https://picsum.photos/seed/demo4/300/300'
    UNION ALL SELECT 5, 'https://www.soundhelix.com/examples/mp3/SoundHelix-Song-5.mp3', 'https://picsum.photos/seed/demo5/300/300'
) t ON true
WHERE u.email = 'demo@blindall.test'
  AND NOT EXISTS (
      SELECT 1 FROM tracks x WHERE x.playlist_id = p.id AND x.provider_id = CONCAT('demo-', t.n)
  );

-- Réponses — Artist (1) + Title (2) pour chaque track démo.
INSERT INTO track_answers (track_id, answer_type_id, value, aliases, score, created_at, updated_at)
SELECT tr.id, '1', 'SoundHelix', jsonb_build_array('sound helix', 'soundhelix'), 0.5, NOW(), NOW()
FROM tracks tr JOIN playlists p ON p.id = tr.playlist_id
WHERE p.name = 'Démo — Hits' AND tr.provider = 'demo'
  AND NOT EXISTS (SELECT 1 FROM track_answers ta WHERE ta.track_id = tr.id AND ta.answer_type_id = '1');

INSERT INTO track_answers (track_id, answer_type_id, value, aliases, score, created_at, updated_at)
SELECT tr.id, '2', REPLACE(tr.provider_id, 'demo-', 'Song '), jsonb_build_array(LOWER(REPLACE(tr.provider_id, 'demo-', 'song '))), 0.5, NOW(), NOW()
FROM tracks tr JOIN playlists p ON p.id = tr.playlist_id
WHERE p.name = 'Démo — Hits' AND tr.provider = 'demo'
  AND NOT EXISTS (SELECT 1 FROM track_answers ta WHERE ta.track_id = tr.id AND ta.answer_type_id = '2');

-- Pages légales — les liens footer pointent vers ces slugs
-- (`route('pages.show', 'mentions-legales')` dans Footer.vue).
INSERT INTO pages (title, slug, content, created_at, updated_at)
SELECT 'Mentions légales', 'mentions-legales', $c$
<h2>Éditeur du site</h2>
<p>Blind'All Music est édité par Clère Kévin — Clere'ly Design.</p>
<p>Contact : <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a> ou via la page <a href="/contact">Contact</a>.</p>
<h2>Hébergement</h2>
<p>IONOS SARL — 7 place de la Gare, 57200 Sarreguemines, France — <a href="https://www.ionos.fr" target="_blank" rel="external nofollow">ionos.fr</a>.</p>
<h2>Propriété intellectuelle</h2>
<p>Les extraits audio joués dans les quiz sont des previews de 30 secondes fournis par les API publiques des plateformes (Deezer, Spotify) ou des extraits sous licence libre uploadés par les modérateurs. Les marques et pochettes appartiennent à leurs ayants droit.</p>
$c$, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM pages WHERE slug = 'mentions-legales');

INSERT INTO pages (title, slug, content, created_at, updated_at)
SELECT 'Politique de confidentialité', 'politique-de-confidentialite', $c$
<h2>Responsable du traitement</h2>
<p>Clère Kévin — Clere'ly Design — <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>.</p>
<h2>Données collectées</h2>
<p>Pseudonyme, adresse e-mail (comptes), scores et classements, messages de chat, historique de parties.</p>
<h2>Connexion sociale</h2>
<p>La connexion via Google ou Discord ne récupère que l'identifiant, le nom et l'adresse e-mail fournis par le service. Aucune donnée n'est publiée sur ces services.</p>
<h2>Finalités</h2>
<p>Fonctionnement du jeu (scores, progression, chat), sécurité des comptes, réponse aux demandes de contact.</p>
<h2>Cookies et stockage local</h2>
<p>Cookie de session technique et jeton d'authentification en stockage local — pas de traceur publicitaire ni d'analytics tiers. Si les annonces sont activées, Google AdSense peut déposer des cookies de mesure et de personnalisation (consentement géré par le bandeau de la plateforme).</p>
<h2>Durées de conservation</h2>
<p>Comptes : jusqu'à suppression par l'utilisateur. Comptes invités : supprimés automatiquement après expiration. Messages de chat : purge périodique automatique.</p>
<h2>Sous-traitants</h2>
<p>Hébergement : IONOS. E-mails transactionnels : Brevo. Dons : Stripe (aucune donnée bancaire n'est stockée par Blind'All Music).</p>
<h2>Vos droits</h2>
<p>Accès, rectification, suppression, portabilité : depuis les paramètres du compte ou à <a href="mailto:clerelydesign@gmail.com">clerelydesign@gmail.com</a>. Réclamation possible auprès de la CNIL (cnil.fr).</p>
<h2>Suppression</h2>
<p>La suppression du compte efface vos données personnelles — depuis les paramètres du compte ou via la page <a href="/contact">Contact</a>.</p>
$c$, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM pages WHERE slug = 'politique-de-confidentialite');
