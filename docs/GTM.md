# Go-to-market & MRR, Blind'All Music

Stratégie de monétisation pour la France : transformer Blind'All de
"projet avec un système de paiement" en "produit qui génère du MRR
récurrent". Toutes les pièces techniques sont en place et vérifiées,
le goulot est la distribution et la conversion, pas le code.

> Les données de marché chiffrées et sourcées sont dans
> `docs/MARKET_STUDY.md` : taille des marchés (jeu vidéo, musique,
> bars), coût réel des animations en bar, dépenses abonnements des
> Français, TikTok, concurrents (Mukiz, Junban, MeloClash...) et
> fourchettes TAM/SAM/SOM.

## État des lieux (honnête)

| Indicateur | Valeur prod (nov. 2026) |
|---|---|
| Utilisateurs | 9 (7 membres, 2 invités) |
| Abonnés payants | 0 (infra Stripe validée, jamais testée en réel) |
| Rooms publiques | 33 (catalogue complet, saisonnalité active) |
| Rounds 30 jours | ~30 |
| Bars Pro | 0 |

Le produit est **pré-lancement** : billing, perks, remboursements,
SEO, saisonnalité, dashboard admin sont finis et testés. Il n'y a
encore aucun revenu et quasi aucun utilisateur. Tout le travail
restant est commercial.

## Positionnement France

**"Blind test" est déjà le mot français.** En France, le "blind test"
est un format de bar connu : un animateur passe des extraits, les
équipes devinent l'artiste ou le titre. Blind'All est la version
digitale de ce format, sans animateur, sans matériel, sur les
téléphones des clients.

- **Produit** : blind test musical multijoueur (rooms publiques,
  duels async, daily challenge, mini-jeux). Gratuit pour tous.
- **Plus, 4,99 €/mois** (joueurs solo) : zéro pub, revive quotidien
  sur le daily, tag personnalisé, perks supporter (couronne,
  réactions chat).
- **Pro, 29 €/mois** (bars, pubs, événements) : tout Plus + branding
  de l'établissement sur l'écran TV (nom + photo de salle), QR code
  vers la room. Cible B2B : un bar abonné = ~6 joueurs Plus en MRR
  équivalent, avec un panier 6x plus gros.

**Le bar est le canal d'acquisition**, pas seulement une cible
payante. Chaque soirée dans un bar Pro expose le jeu à 20-50 joueurs
qui deviennent utilisateurs (et potentiellement Plus) demain.

## Pourquoi ça peut marcher en France (chiffré)

1. **Le format est culturellement ancré** : le blind test en bar
   existe déjà, les gens connaissent la règle. Pas d'éducation marché.
2. **Zéro friction d'installation** : le client scanne un QR code et
   joue dans le navigateur. Pas de download, pas de compte obligatoire
   (guest upgradeable). C'est l'argument anti-animateur parfait.
3. **Le prix est dans l'impulsion** : 4,99 €/mois est sous le seuil
   psychologique des 5 € et dans la fourchette observée des apps
   payantes FR (3-5 €/mois en moyenne, 32 % des mobinautes ont déjà
   payé une app). Un café par mois.
4. **La concurrence bar est très chère** : les prestataires français
   facturent **300-900 € TTC par soirée** de blind test animé
   (QuizMania, L'Heure Bleue, MyAnimation, PeachProduction, Danomic).
   Pro à 29 €/mois = **3-10 % d'une seule soirée**, illimité. C'est
   l'argument de vente n°1.
5. **Marchés porteurs** : 40 M de joueurs FR (76 % jouent chaque
   semaine, âge moyen 40 ans), 18,7 M d'utilisateurs de streaming
   musical (pénétration 27 %, non saturée), ~44 800 débits de
   boissons majoritairement indépendants à décideur unique.
6. **La France est le 1er marché TikTok d'Europe** (27,8 M
   d'utilisateurs/mois) : canal d'acquisition B2C à coût nul pour
   un format "son + suspense + compétition" natif.
7. **Un calendrier marketing déjà construit** : les rooms
   saisonnières (Noël, Halloween, Eurovision, Saint-Patrick, été)
   pilotent automatiquement la visibilité. Chaque saison est un
   pic de communication clé en main.
8. **La concurrence directe est contournable** : Mukiz (1M+
   downloads) prouve le marché mais ne fait ni rooms persistantes,
   ni daily, ni mini-jeux, ni duels async, ni saisonnalité, ni mode
   bar/TV. Ne pas courir après ses features : occuper le terrain
   "blind test de bar digital" qu'il ne couvre pas.

## Entonnoir de conversion

```
Trafic (bars, TikTok, Discord, SEO, stores)
   → landing (/ ou /blind-test-*, /daily, /duels)
   → première partie (guest OK, zéro friction)
   → inscription (compte pour garder son score)
   → parties quotidiennes (daily + rooms)
   → friction pub / envie de perks
   → /premium → case CGV → Stripe Payment Link
   → webhook → perks actifs immédiatement
   → Customer Portal pour gestion/résiliation
```

Points de contact payants en place : navbar, home (pitch card),
footer, `/premium`, `/docs/support`, `/bars` (argumentaire B2B
complet), settings (bloc abonnement + CTA perks), bannière
"Me support". Pages légales FR complètes : CGV, CGU, mentions
légales, politique de confidentialité.

## Canaux, par ordre de ROI attendu

### 1. Bars & événements (Pro), priorité absolue

C'est le canal le plus rentable ET le plus fort en acquisition.

**Segments cibles** :
- Bars à quiz / bars à thème existants : ils connaissent le format,
  ils cherchent un outil digital moins cher qu'un animateur.
- Bars de jeu et bars à cocktails : clientèle qui aime les
  animations interactives.
- Bars étudiants (Lyon, Lille, Bordeaux, Toulouse, Montpellier,
  Nantes, Strasbourg, Rennes) : volume élevé, budget sensible,
  forte affinité jeu.
- Licence IV / bars de nuit : besoin d'animation les soirs creux
  (dimanche-mardi).
- Événements : team building, soirées associatives, festivals
  locaux, soirées d'entreprise (via agences d'animation).

**Proposition commerciale** (appuyée par les tarifs observés) :
- Offre de lancement : **3 premiers bars à 19 €/mois pendant 3 mois**
  (tarif d'entrée), ensuite 29 €/mois. Objectif : 5 bars sous 90 jours
  = ~95-145 €/mois de MRR B2B.
- Argumentaire chiffré : "Une soirée blind test animée vous coûte
  300-900 € TTC (prestataires FR). Blind'All Pro = 29 €/mois,
  illimité, sur l'écran et les téléphones que vous avez déjà."
- Nuance honnête : les prestataires vendent aussi l'animateur et le
  spectacle. Le pitch correct est "animez vos soirs creux sans
  animateur", pas "remplacez votre animation".
- Démo : créer une room publique de démo par ville, envoyer le lien.
  Le prospect voit le jeu en 30 secondes sans install.
- Marché : ~44 800 débits de boissons, 95 % indépendants, 61 % sans
  salarié : des commerçants qui décident seuls, avec un cycle de
  vente court. Pas de comité, mais budget serré : la simplicité
  de l'offre est un argument.

**Canaux d'outreach concrets** :
- Instagram/TikTok : DM aux pages de bars (recherche "bar quiz",
  "blind test", "bar à thème" + ville).
- Groupes Facebook : "animateurs de soirée quiz", "organisateurs
  d'animations bar", réseaux d'animations locales.
- Google Maps : liste des bars avec "quiz", "blind test", "karaoke"
  dans leurs avis/description, puis email ou visite.
- LinkedIn : agences d'animation, organisateurs d'événements
  d'entreprise (pro positionnement Pro).
- CCI et réseaux d'animation municipaux : permanences, salons
  des associations.
- Partenariat blanc possible plus tard : les concepts "blind test"
  clés en main (franchises) pourraient adopter l'outil.

**Argumentaire page `/bars`** (déjà en ligne) : zéro matériel,
mode TV grand écran, QR code, playlists officielles, scoreboard
temps réel, branding établissement, photo de salle.

### 2. TikTok / YouTube Shorts / Instagram (acquisition B2C)

Le produit est **visuellement partageable** : extraire des clips de
parties folles (mauvaise réponse du groupe, comeback en daily,
suspense sur une intro).

**Formats qui marchent en FR** :
- "On a fait un blind test entre potes, regarde ce qui s'est passé"
  (screen recording + réactions).
- "Devine la suite" : clip musical + suspense, le jeu en fond.
- POV bar : "ton blind test au bar mais versionextrême".
- Challenges saisonniers calés sur les rooms : "l'épreuve chantée
  de Noël en blind test" en novembre-décembre.
- Face caméra : streamer/réaction sur les réponses absurdes du
  groupe (l'audio des joueurs fait le contenu).

**Cadence** : 1 vidéo/semaine minimum, pas de stratégie complexe.
Mesurer avec `?utm_source=tiktok` / `utm_campaign=shorts` sur les
liens (à vérifier : les UTM ne sont pas encore trackés côté analytics,
le dashboard admin donne les volumes globaux).

### 3. Discord & communautés existantes

- Serveur Discord officiel déjà lié (footer). Annoncer le lancement
  Premium aux early adopters : ce sont les utilisateurs les plus chauds.
- Serveurs Discord FR de musique, de jeux, de quiz : proposer une
  room privée pour tester, mesure d'audience avant tout post.
- Communautés "blind test" Telegram/Discord existantes : le produit
  est leur version digitale officielle.

### 4. SEO & pages publiques (déjà en place, à alimenter)

Pages indexables existantes : `/premium`, `/bars`, `/duels`,
`/daily`, `/docs/*`, `/blind-test-{slug}` (landings catégories avec
vrai copy SEO), `/games/{type}`, sitemap dynamique avec saisonnalité.

**Mots-clés FR prioritaires** :
- "blind test en ligne", "blindtest en ligne", "blind test gratuit"
- "quiz musical multijoueur", "jeu blind test entre amis"
- "blind test pop / années 80 / chanson française / electro" (déjà
  couverts par les slugs de rooms)
- "blind test bar" (B2B, page `/bars`)

**Actions** :
- Contenu régulier : articles "meilleurs blind test en ligne",
  comparatifs, pages par genre. Commencer par 2-3 pages à fort
  intent.
- La saisonnalité est un atout SEO : les landings événementielles
  (Noël, Halloween) ont un pic de recherche naturel chaque année.
  Préparer les posts quand la room devient visible.

### 5. Stores (Google Play + iOS)

Plan documenté dans `docs/STORES.md` (TWA Bubblewrap Android,
PWABuilder iOS). Coût : 25 $ Google + 99 $/an Apple.

- Le store est le plus gros levier de **découverte organique** pour
  un jeu grand public : "blind test" est une recherche store réelle.
- Ordre : **Android d'abord** (25 $ one-shot, marché FR large),
  iOS ensuite si les métriques le justifient.
- SEO store : titre "Blind Test : quiz musical multijoueur", screenshots
  de parties, description FR (EN plus tard).

### 6. PWA & bouche-à-oreille

- La PWA a déjà une invite d'installation ("Installer l'application").
  En France, "ajouter à l'écran d'accueil" est un réflexe connu :
  zéro friction store.
- Chaque room publique est un vecteur : le joueurinvite un ami,
  la boucle grandit seule.
- Système de parrainage éventuel plus tard (quand MRR > 0).

### 7. Partenariats & presse (secondaire)

- Blogs/médias FR de jeux vidéo et de culture pop : pitch "le blind
  test de bar, en ligne et gratuit".
- Podcasts FR de musique/jeux : format "on a testé le blind test
  en ligne entre nous".
- Communautés d'"animateurs de soirée" : ils sont les prescripteurs
  naturels du format en bar.

## Calendrier marketing saisonnier (déjà construit)

Les rooms saisonnières pilotent automatiquement la visibilité.
Aligner la communication dessus :

| Période | Room visible | Angle com' |
|---|---|---|
| Octobre | Halloween | "Le blind test d'Halloween" |
| Novembre-décembre | C'est Noël | Pic recherche "blind test noël" |
| Décembre-janvier | Nouvel an | Rétrospective année + résolution |
| Mars | Saint-Patrick | Angle fête/green, communauté IRL |
| Mai-juin | Eurovision | Suivi de l'Eurovision (très FR) |
| Été | Summer hits | Playlists estivales |

Chaque changement de saison = une excuse pour poster (TikTok,
Discord, communities). Préparer les assets quand la room devient
visible dans le sitemap.

## Pricing & packaging, décisions actées

| Plan | Prix | Cible | Rôle |
|---|---|---|---|
| Gratuit | 0 € | Tous | Acquisition, volume, funnel |
| Plus | 4,99 €/mois | Joueurs engagés | MRR volume (long tail) |
| Pro | 29 €/mois (19 € promo lancement) | Bars/événements | MRR qualité |

- **Pas de plan don** : retiré, créait de la confusion avec les
  abonnements et ne générait rien (0 don en prod).
- **Pas de plan annuel** pour l'instant : valider la rétention
  mensuelle d'abord. Réévaluer à 50+ abonnés (offre -20 % annuel).
  Signal concurrent : Mukiz ne propose que annuel + hebdomadaire
  après ses tests A/B (le mensuel a été retiré). Si notre rétention
  mensuelle déçoit, ne pas hésiter à sauter l'étape intermédiaire.
- **AdSense reste sur le gratuit** : monétisation indirecte des
  non-payants, et argument Plus concret ("zéro pub").
- Le prix Pro à 29 € est volontairement sous le seuil "un repas"
  pour un commerce : décision facile pour un gérant de bar.

## Remboursements & droit de rétractation (B2C)

- **Base légale** : art. L221-18 C. conso, 14 jours de rétractation.
  Pour du contenu numérique exécuté immédiatement, le droit peut
  être levé par **renonciation expresse** (art. L221-28 13°) : c'est
  la case à cocher des pages `/premium` et `/docs/support`, reprise
  dans les CGV (art. 6). Les boutons Stripe restent verrouillés
  tant que la case n'est pas cochée.
- **Révocation automatique** : un remboursement intégral dans le
  dashboard Stripe déclenche `charge.refunded` → souscription
  `canceled`, perks retirés (ads, revive, tag, venue Pro),
  notification e-mail. Résolution par `stripe_invoice_id` avec
  fallback customer Stripe.
- **Remboursement partiel** : ignoré volontairement, un geste
  commercial ne doit pas couper l'abonnement.
- **Procédure ops** : refund dans Stripe puis résiliation (ou
  portail client). Le webhook couvre les deux chemins.
- **B2B (bars)** : les clients pro ne bénéficient pas du droit de
  rétractation grand public de la même façon (client professionnel) ;
  les CGV s'appliquent. Vérifier le wording CGV pour la distinction
  B2C/B2B si des bars s'abonnent.

## KPIs à suivre

Le **dashboard admin** (`/admin`) affiche maintenant les métriques
en direct : users, rooms, subscriptions, activité 7j/30j, contenu,
top rooms, plus l'historique (snapshots horaires) et l'export CSV.

Fourchettes SOM de l'étude de marché (12 mois) : **~1 000-4 500 €/mois
MRR** (5-15 bars Pro + 200-800 Plus). Le bas de fourchette est le
scénario de référence à viser ; le haut suppose que le bouche-à-oreille
joue.

| Métrique | Cible 90 jours | Où |
|---|---|---|
| Utilisateurs actifs hebdo | 500+ | dashboard admin (activity) |
| Taux free → inscription | > 30 % | auth (users guests vs membres) |
| Taux inscription → payant | > 2 % | user_subscriptions |
| MRR | > 300 € | dashboard (subscriptions) |
| Bars Pro | 3-5 | rooms.is_pro |
| Churn mensuel | < 15 % | subscription.deleted events |
| Nouveaux users / semaine | croissance | dashboard history |

Requête MRR rapide :

```sql
SELECT plan, COUNT(*) AS subs,
       COUNT(*) * CASE plan WHEN 'pro' THEN 29.00 ELSE 4.99 END AS mrr_eur
FROM user_subscriptions
WHERE status IN ('active','trialing','past_due')
GROUP BY plan;
```

## Roadmap 90 jours

**Semaines 1-2 (fondations, fait)** :
- Billing Stripe complet et vérifié : payment links live, webhook
  signature OK, portail client, refunds auto, perks testés
  (free/plus/pro validés, comptes de test supprimés).
- SEO, saisonnalité, catalogue, dashboard admin : déployés.
- Reste manuel : **1 paiement test réel 4,99 € + remboursement**
  pour valider le tunnel Stripe de bout en bout, et vérifier le
  statut du site dans la console AdSense.

**Semaines 2-6 (premiers revenus)** :
- Outreach bars : 30 contacts (Instagram/Maps/FB), 3 démos de rooms
  par ville cible. Objectif : 1-2 bars en offre lancement 19 €.
- TikTok : 4-6 posts courts (screen recordings de parties), UTM
  sur les liens.
- Annonce Discord : lancement Premium aux early adopters.
- Soumission Google Play (TWA Android).

**Mois 2 (itération)** :
- Itérer sur les objections pricing/features remontées par les bars.
- 2-3 pages SEO à fort intent.
- iOS si Android validé.
- Premier bar Pro plein tarif : le cas client à documenter pour
  la prospection suivante.

**Mois 3 (scale ou pivot)** :
- Décision : plan annuel, parrainage, ou feature Pro n°2
  (sonorisation/quiz house depuis la room ?).
- Si 0 bar après 60 contacts : revoir le ciblage (peut-être les
  animateurs de soirée plutôt que les bars, ou les événements
  associatifs d'abord).
- Churn check : l'étude montre que 44 % des Français ont déjà
  oublié de résilier un abo et que le produit doit donner une
  raison de revenir chaque jour. Regarder la courbe d'activité
  du dashboard : si les users payants ne jouent plus après 2
  semaines, le problème est la valeur perçue, pas le prix.

## Ce qui n'est PAS dans le scope

- Reconstruction du billing : Stripe Payment Links + webhooks +
  Customer Portal suffisent à cette échelle.
- Programme d'affiliation, facturation entreprise, multi-devises :
  prématuré avant 100+ abonnés.
- Marketing payant (Ads, influenceurs) : burn sans produit-prouvé.
- Contenu audio/licences : le catalogue Deezer preview + playlists
  existantes suffisent pour la validation ; les questions de
  droits à grande échelle se poseront après traction.

## Checklist lancement (prochaine session)

- [ ] Paiement test réel 4,99 € sur `/premium` + refund Stripe,
      vérifier perks activés puis retirés.
- [ ] Console AdSense : statut du site (approuvé ? démarrage des
      impressions ?).
- [ ] Stripe dashboard : *Terms of service URL* →
      `/pages/conditions-generales-vente` (cible du checkbox CGV).
- [ ] Pitch bar type : 1 page PDF/Notion avec screenshots + offre
      19 €/mois pour les 3 premiers.
- [ ] 30 noms de bars cibles + canaux de contact (première liste).
- [ ] 4 concepts de posts TikTok/Shorts filmables en 1 journée.
