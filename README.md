# Blind'All Music

> Blind-test musical multijoueur en temps réel — reconstruit entièrement en Rust.

Quiz musicaux en ligne, gratuits et multijoueurs : écoute l'extrait, devine le
titre ou l'artiste avant les autres, grimpe au classement ELO.

## Fonctionnalités

- **Rooms multijoueur** — parties en temps réel, scoring piloté par le serveur
  (artiste, titre, featuring, et œuvre pour jeux vidéo / films / séries / animés)
- **Mini-jeux solo** — quiz 4 choix, qui a chanté, anagramme, première lettre,
  pochette d'album
- **Progression** — XP, niveaux, classement ELO, badges, équipes
- **Catalogue** — imports Spotify, Deezer, iTunes, Audius, YouTube + titres locaux
- **Communauté** — chat en salle, réactions, signalements, modération
- **Auth** — comptes invités convertibles, OAuth Google / Discord, reset mot de
  passe, vérification d'e-mail
- **Complet** — i18n fr/en/es, SEO (sitemap, JSON-LD), donations Stripe,
  notifications e-mail Brevo, admin back-office

## Stack

| Couche | Tech |
|---|---|
| Backend | Axum · SQLx · Tokio — API REST `/api/v1`, WS `/ws` (Echo-compatible), gameplay serveur |
| Frontend | Leptos (WASM) — SPA, routage client |
| Données | PostgreSQL (image `supabase/postgres`) · Redis |
| E-mails | Brevo HTTP API |
| Dons | Stripe |
| Déploiement | Docker Compose + Caddy (TLS Let's Encrypt auto) |

## Arborescence

```
crates/
  blindall-api        endpoints REST, auth, OAuth, SEO, admin
  blindall-core       types & helpers partagés
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

## Production

```sh
docker compose --env-file .env.production \
  -f docker-compose.yml -f docker-compose.prod.yml up -d
```

Caddy sert `https://blindallmusic.com` (TLS auto). Variables requises : voir
`.env.example` (Brevo, OAuth Google/Discord, Spotify ; YouTube/Stripe optionnels).

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

Voir [SECURITY.md](SECURITY.md) — ne jamais rapporter une vulnérabilité en
issue publique.

## Licence

Propriétaire — tous droits réservés. Voir [LICENSE](LICENSE).

## Note Windows (toolchain sans MSVC)

Ce workspace se compile sous `x86_64-pc-windows-gnullvm` (rust-lld self-contained).
Si la machine n'a pas de toolchain C : `python tools/gen_import_libs.py` régénère
`.libs/` (import libs MinGW), puis restaurer `.cargo/config.toml` (voir
`tools/cargo-config-gnullvm.toml`).
