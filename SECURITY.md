# Politique de sécurité

## Signaler une vulnérabilité

**Ne pas ouvrir d'issue publique.** Écris à **clerelydesign@gmail.com** avec :

- description de la vulnérabilité et de son impact,
- étapes de reproduction ou PoC,
- version / commit concerné.

Accusé de réception sous 72 h, correctif prioritaire selon la sévérité.

## Périmètre

- `blindall-server` et l'API `/api/v1` (auth, rooms, gameplay, admin)
- Le hub WebSocket `/ws` (autorisation des channels, presence)
- La chaîne de déploiement (Dockerfile, Compose, Caddyfile)

## Hors périmètre

- Les services tiers (Spotify, Deezer, Brevo, Stripe), remonter à l'éditeur concerné
- Le contenu généré par les utilisateurs (hors faille applicative)

## Bonnes pratiques du projet

- Secrets uniquement via `.env` / `.env.production`, jamais commités
- Postgres et Redis non exposés publiquement (loopback / réseau interne)
- Mots de passe bcrypt, sessions par jeton, OAuth Google/Discord
