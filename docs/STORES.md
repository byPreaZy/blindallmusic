# Publication sur les stores — Google Play & iOS

Blind'All est une PWA complète (manifest, service worker, icônes maskable,
mode standalone). Les deux stores se couvrent sans réécriture :

- **Android** : TWA (Trusted Web Activity) — le site en plein écran dans
  une coquille Android signée, zéro code natif à maintenir.
- **iOS** : package PWABuilder (WKWebView) — nécessite Xcode sur Mac.

## Prérequis

| Besoin | Coût | Notes |
|---|---|---|
| Compte Google Play Console | 25 $ unique | https://play.google.com/console |
| Compte Apple Developer | 99 $/an | https://developer.apple.com |
| Node.js + JDK 17 + Android SDK | gratuit | pour Bubblewrap (Android) |
| Mac avec Xcode | — | uniquement pour iOS |

Le package Android suggéré : `fr.clerely.blindall` (reverse-domain de
`clerely.fr`, que tu possèdes).

## Android — TWA via Bubblewrap

```bash
npm install -g @bubblewrap/cli
mkdir blindall-twa && cd blindall-twa
bubblewrap init --manifest https://blindallmusic.clerely.fr/manifest.json
# Répondre : package fr.clerely.blindall, nom Blind'All Music,
# icônes auto-générées depuis le manifest, key store : créer un nouveau
bubblewrap build    # produit app-release-signed.apk / .aab
```

Bubblewrap génère un projet Android Studio complet (à garder hors du repo
ou dans un repo dédié — il ne change qu'en cas de bump de version).

### Lier le domaine (obligatoire, sinon barre d'URL visible)

1. Sous Play Console → **Signature de l'app → Intégrité de l'app**,
   récupère l'empreinte **SHA-256 du certificat de signature**.
2. Sur le VPS, dans `~/blindall/.env.production` :

   ```
   ANDROID_PACKAGE_NAME=fr.clerely.blindall
   ANDROID_SHA256_FINGERPRINT=AB:CD:…:EF
   ```

3. Redéployer → vérifier `GET /.well-known/assetlinks.json` renvoie le
   JSON Digital Asset Links (200 + application/json).
4. Uploader l'.aab → l'app s'ouvre en plein écran sur le domaine.

Pour un test local avant Play signing, utiliser l'empreinte du keystore
d'upload (`keytool -list -v -keystore upload.keystore`) dans la même var.

### Listing Play Console

- Icône 512×512 : `web/public/favicon-512x512.png` (déjà dans le repo)
- Feature graphic 1024×500 : à créer (Canva/Figma, logo + tagline)
- 2+ captures téléphone : depuis le site en mode responsive ou l'émulateur
- Catégorie : Jeux → Quiz ; Contenu : PEGI 3
- Lien politique de confidentialité (obligatoire) :
  https://blindallmusic.clerely.fr/pages/politique-confidentialite
- Déclarer les pubs (AdSense) et les achats intégrés (Stripe s'ouvre dans
  le navigateur externe → pas de Play Billing requis).

## iOS — PWABuilder

1. https://www.pwabuilder.com → entrer `https://blindallmusic.clerely.fr`
2. Onglet **iOS** → télécharger le package Xcode généré
3. Sur Mac : ouvrir dans Xcode → régler `Bundle ID`
   (`fr.clerely.blindall`) → Archive → Upload App Store.

⚠️ **Directive Apple 4.2 (Minimum Functionality)** : une coquille web pure
peut être refusée. Pour maximiser les chances : remplir la fiche avec les
fonctionnalités réelles (quiz multijoueur temps réel, classements, mode
TV + pad), activer les notifications push iOS si possible, et présenter
l'app comme un jeu, pas comme un wrapper de site. Le mode "Ajouter à
l'écran d'accueil" Safari fonctionne déjà comme fallback immédiat.

## Copy listing (FR)

- **Titre** : Blind'All Music — Blind tests multijoueurs
- **Description courte** : Le blind test géant entre amis, en direct.
- **Description longue** : Crée une room en 10 secondes, partage le lien,
  et affronte tes amis sur des extraits musicaux : années 80, Disney, rap,
  rock, chanson française… Buzz depuis ton téléphone, affiche les scores
  sur la TV. Quiz quotidien, classements, niveaux et défis entre joueurs.
- **Mots-clés** : blind test, quiz musical, quiz musique, blindtest,
  multijoueur, party game.
