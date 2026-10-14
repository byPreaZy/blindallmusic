# Contribuer à Blind'All Music

Merci de l'intérêt ! Le projet est **propriétaire**, toute contribution implique
le transfert des droits sur le code soumis (patch, PR) à l'éditeur du projet.

## Avant de commencer

- Ouvre une **issue** avant tout gros changement (nouvelle feature, refactor).
  on valide l'approche ensemble.
- Petits fixes (typo, bug évident) : PR directe OK.

## Setup

```sh
cp .env.example .env
docker compose up -d                 # stack complète sur :8080
cargo test --workspace               # tests unitaires
cargo test -p blindall-e2e --test '*' # e2e contre le serveur local
```

## Conventions

- **Rust** : `cargo fmt` + `cargo clippy --workspace` propres avant PR.
- **Frontend** (Leptos) : textes utilisateur via `t("clé")` dans `web/src/i18n.rs`
  : jamais en dur. Classes Tailwind existantes de préférence (palette `brand-*`).
- **i18n** : toute nouvelle chaîne UI doit exister en `fr`, `en`, `es`.
- **Gameplay** : le client audio est *display-only*, timing et transitions sont
  pilotés par le serveur (`blindall-game`). Ne jamais déplacer la logique de jeu
  côté client.
- **Commits** : messages courts, impératif, en anglais (`Fix X`, `Add Y`).
  Pas de footers de génération.
- **Secrets** : aucun secret, clé ou dump de données dans le repo, jamais.

## Tests

- Nouvelle feature → test e2e ou unitaire couvrant le comportement.
- Bugfix → le test qui échouait avant, qui passe après.
- `cargo test --workspace` et la suite e2e doivent rester vertes.

## Pull Request

- Une PR = un changement cohérent. Décris le *pourquoi*, pas juste le *quoi*.
- UI : joins des captures d'écran (desktop + mobile).
- Vérifie que rien de généré n'est commité (`target/`, `dist/`, `*.env`,
  `prod_seed.sql`, `volumes/`).

## Signalement de vulnérabilités

Pas en issue publique : voir [SECURITY.md](SECURITY.md).
