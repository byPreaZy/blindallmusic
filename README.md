# Blind'All Music

> Blind-test musical multijoueur en temps réel, reconstruit entièrement en Rust.

Quiz musicaux en ligne, gratuits et multijoueurs : écoute l'extrait, devine le
titre ou l'artiste avant les autres, grimpe au classement ELO.

**Production : <https://blindallmusic.clerely.fr>**

## Fonctionnalités

- **Rooms multijoueur** : parties en temps réel, scoring piloté par le serveur
  (artiste, titre, featuring, et œuvre pour jeux vidéo / films / séries / animés)
- **Mini-jeux solo** : quiz 4 choix, qui a chanté, anagramme, première lettre,
  pochette d'album
- **Progression** : XP, niveaux, classement ELO multijoueur zero-sum, badges,
  équipes, saisons hebdomadaires avec récompenses
- **Classements** : ladder global (ELO / score / niveau / semaine), podium par
  room (all-time / équipes / 7 jours), board saison
- **PWA** : installable mobile, cache hors-ligne des assets et des extraits
  audio déjà joués (parfait dans le métro)
- **Catalogue** : imports Spotify, Deezer, iTunes, Audius, YouTube + titres locaux
- **Communauté** : chat en salle, réactions, signalements, modération
- **Auth** : comptes invités convertibles, OAuth Google / Discord, reset mot de
  passe, vérification d'e-mail
- **Complet** : i18n fr/en/es, SEO (sitemap, JSON-LD), donations Stripe,
  e-mails transactionnels SMTP, admin back-office

## Stack

| Couche | Tech |
|---|---|
| Backend | Axum · SQLx · Tokio, API REST `/api/v1`, WS `/ws` (Echo-compatible), gameplay serveur |
| Frontend | Leptos (WASM), SPA, routage client, PWA (service worker) |
| Données | PostgreSQL (image `supabase/postgres`) · Redis |
| E-mails | SMTP générique via `lettre` (`MAIL_*`, n'importe quel provider) |
| Dons | Stripe |
| Déploiement | Docker Compose + Traefik (ou Caddy) — TLS Let's Encrypt auto |

## Arborescence

```
crates/
  blindall-api        endpoints REST, auth, OAuth, SEO, admin
  blindall-core       types & helpers partagés (ELO, matching, levels)
  blindall-db         accès données (SQLx)
  blindall-game       acteur de jeu (rounds, timing, scoring)
  blindall-providers  Spotify / Deezer / iTunes / Audius / YouTube / local
  blindall-realtime   hub WebSocket, channels, presence
  blindall-redis      bus pub/sub + cache
  blindall-server     binaire : wiring, scheduler, sweeper
  blindall-e2e        tests end-to-end HTTP/WS
migrations/           schéma SQLx
storage/seed/         covers des rooms officielles
tools/                seed prod, helpers
web/                  crate Leptos + Tailwind + assets
.github/workflows/    CI/CD : build -> transfert SSH -> compose up sur le VPS
```

## Développement

```sh
cp .env.example .env          # valeurs par défaut fonctionnelles en local
docker compose up -d          # db + redis + server → http://localhost:8080
```

Tests :

```sh
cargo test --workspace                    # unitaires
cargo test -p blindall-e2e --test '*'     # e2e (BASE_URL, défaut localhost:8080)
```

Astuce dev : un Mailpit local (`docker run -d -p 8025:8025 -p 1025:1025
axllent/mailpit`, `MAIL_HOST=localhost MAIL_PORT=1025 MAIL_ENCRYPTION=none`)
permet de lire les e-mails envoyés sans rien configurer.

## Production

Deux options de reverse proxy (TLS automatique dans les deux cas) :

```sh
# Avec Traefik existant sur l'hôte :
docker compose --env-file .env.production \
  -f docker-compose.yml -f docker-compose.prod.yml \
  -f docker-compose.traefik.yml up -d

# Ou avec le Caddy embarqué :
docker compose --env-file .env.production \
  -f docker-compose.yml -f docker-compose.prod.yml up -d
```

Variables requises : voir `.env.example` (SMTP `MAIL_*`, OAuth Google/Discord,
Spotify ; YouTube/Stripe/AdSense optionnels). Le service `cloudflared`
(`--profile tunnel`) permet une URL éphémère `*.trycloudflare.com` sans DNS.

### CI/CD

`.github/workflows/deploy.yml` : sur push `main`, build de l'image Docker sur le
runner, `docker save | ssh | docker load` vers le VPS, puis `compose up`.
Secrets requis : `VPS_HOST`, `VPS_USER`, `VPS_SSH_KEY`, `ENV_PRODUCTION`.

Données officielles (rooms, playlists, tracks enrichies) :

```sh
./tools/make_prod_seed.sh              # génère prod_seed.sql depuis la DB de dev
psql "$DATABASE_URL" -f prod_seed.sql  # après les migrations, sur base fraîche
./tools/seed_storage.sh                # covers des rooms → volume storage
```

## Contribuer

Voir [CONTRIBUTING.md](CONTRIBUTING.md). Bugs et suggestions : ouvrir une issue
ou passer par la page Contact du site.

## Sécurité

Voir [SECURITY.md](SECURITY.md) : ne jamais rapporter une vulnérabilité en
issue publique.

## Licence

Propriétaire : tous droits réservés. Voir [LICENSE](LICENSE).

## Note Windows (toolchain sans MSVC)

Ce workspace se compile sous `x86_64-pc-windows-gnullvm` (rust-lld self-contained).
Si la machine n'a pas de toolchain C : `python tools/gen_import_libs.py` régénère
`.libs/` (import libs MinGW), puis restaurer `.cargo/config.toml` (voir
`tools/cargo-config-gnullvm.toml`).
