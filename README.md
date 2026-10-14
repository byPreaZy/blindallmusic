# Blind'All Music

Blind-test musical multijoueur en temps réel.

## Stack

- **Backend** : Axum · SQLx · Tokio — API REST `/api/v1`, WebSocket `/ws` (événements Echo-compatibles), jeu piloté serveur
- **Frontend** : Leptos (WASM) — SPA avec routage client, i18n fr/en/es
- **Données** : PostgreSQL (+ Supabase optionnel via profil Compose) · Redis
- **Providers musique** : Spotify · Deezer · iTunes · Audius · YouTube · uploads locaux
- **OAuth** : Google · Discord
- **Déploiement** : Docker Compose + Caddy (TLS Let's Encrypt)

## Développement

```sh
cp .env.example .env          # remplir les valeurs
docker compose up -d          # db + redis + server → http://localhost:8080
```

Tests :

```sh
cargo test --workspace                        # unitaires
cargo test -p blindall-e2e --test '*'         # e2e contre le serveur (BASE_URL par défaut : localhost:8080)
```

## Production

```sh
docker compose --env-file .env.production -f docker-compose.yml -f docker-compose.prod.yml up -d
```

Caddy sert `https://blindallmusic.com` (TLS auto). Voir `.env.production` pour les variables requises.

Données officielles (rooms, playlists, tracks enrichies) :

```sh
./tools/make_prod_seed.sh              # génère prod_seed.sql depuis la DB de dev
psql "$DATABASE_URL" -f prod_seed.sql  # après les migrations, sur une base fraîche
./tools/seed_storage.sh                # injecte les covers des rooms dans le volume storage
```

## Note Windows (toolchain sans MSVC)

Ce workspace se compile sous `x86_64-pc-windows-gnullvm` (rust-lld self-contained).
Si la machine n'a pas de toolchain C : `python tools/gen_import_libs.py` régénère `.libs/`
(import libs MinGW), puis restaurer `.cargo/config.toml` (voir historique git / copier
depuis `tools/` si absent).
