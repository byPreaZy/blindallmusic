# Go-to-market & MRR — Blind'All Music

Stratégie de monétisation : faire passer Blind'All de "projet avec un
système de paiement" à "produit qui génère du MRR récurrent". Toutes les
pièces techniques existent déjà — le goulot est la distribution et la
conversion, pas le code.

## Positionnement

- **Produit** : blind test musical multijoueur (rooms publiques, duels
  async, daily challenge, mini-jeux). Gratuit pour tout le monde.
- **Plus — 4,99 €/mois** ( joueurs solo ) : zéro pub, revive quotidien sur
  le daily, tag personnalisé, tous les perks supporter (couronne,
  réactions chat).
- **Pro — 29 €/mois** ( bars, pubs, événements ) : tout Plus + branding de
  l'établissement sur l'écran TV (nom + photo de salle), QR code vers la
  room. Cible B2B : un seul bar abonné = ~6 joueurs Plus en MRR équivalent.

Le Positionnement clé : **le bar est le canal d'acquisition**, pas une
simple cible payante. Chaque soirée dans un bar Pro expose le jeu à 20-50
joueurs qui peuvent devenir des utilisateurs (et des Plus) demain.

## Entonnoir de conversion

```
Trafic (stores, SEO, TikTok, bars)
   → inscription (compte ou guest → converti)
   → parties quotidiennes (rétention)
   → friction pub / envie de perks
   → /premium → Stripe Payment Link
   → webhook → perks actifs immédiatement
   → Customer Portal pour gestion/résiliation
```

Points de contact payants déjà en place : navbar, home (pitch card),
footer, `/premium`, `/docs/support`, page `/bars` (argumentaire B2B),
settings (bloc abonnement + CTA perks), bannière "Me support CTA".

## Canaux, par ordre de ROI attendu

### 1. Bars & événements (Pro) — priorité absolue

- **Cible** : bars à quiz/trivia existants, bars de jeu, soirées team
  building, événements associatifs. Ils ont déjà l'habitude de payer pour
  animer une salle (un blind test = zéro matériel, zéro animation).
- **Action** : liste de 30-50 établissements FR (Discord, groupes
  "animateurs de soirée quiz", CCI, salons du jeu). Outreach direct par
  email/DM avec la page `/bars` + une room de démo publique à rejoindre.
- **Offre de lancement** : 3 premiers bars à 19 €/mois pendant 3 mois
  (prix d'entrée Startup/Bar), ensuite tarif plein. Objectif : 5 bars
  sous 90 jours = ~145 €/mois de MRR B2B seul.
- **Argumentaire** : vs. un animateur payé (150-300 €/soir) ou un quiz
  box (matériel + abonnement), Pro coûte moins qu'une soirée.

### 2. Stores (Google Play + iOS)

- Plan déjà documenté dans `docs/STORES.md` (TWA Bubblewrap Android,
  PWABuilder iOS). Coût : 25 $ Google + 99 $/an Apple.
- Le store est le plus gros levier de **découverte organique** pour un
  jeu grand public : "blind test" est une recherche store réelle.
- SEO store : titre "Blind Test : quiz musical multijoueur", screenshots
  de parties épiques, FR + EN.

### 3. SEO & pages publiques (déjà en place, à alimenter)

- Pages indexables existantes : `/premium`, `/bars`, `/duels`, `/daily`,
  `/docs/*`, `/blind-test-{slug}` (category landings).
- Ajouter du contenu régulier : articles "meilleurs blind test en ligne",
  comparatifs, pages par genre musical. C'est le canal le moins coûteux
  sur le long terme.

### 4. Contenu court (TikTok / YouTube Shorts / Instagram)

- Le produit est **visuellement partageable** : extraire des clips de
  parties folles (mauvaise réponse du groupe, победа en daily).
- Format : "on a fait un blind test dans un bar, regarde ce qui s'est
  passé" — boucle de partage native.
- Objectif : 1 vidéo/semaine, pas de stratégie complexe. Mesurer via
  UTM `?utm_source=tiktok` sur les liens (à ajouter si absent).

### 5. Discord & communauté existante

- Serveur Discord déjà lié (footer). Annoncer le lancement Premium aux
  early adopters — les 7 users actuels sont les plus chauds.
- Système de parrainage éventuel plus tard (quand MRR > 0).

## Pricing & packaging — décisions actées

| Plan | Prix | Cible | Rôle |
|---|---|---|---|
| Gratuit | 0 € | Tous | Acquisition, volume, funnel |
| Plus | 4,99 €/mois | Joueurs engagés | MRR volume (long tail) |
| Pro | 29 €/mois | Bars/événements | MRR qualité (peu de clients, gros panier) |

- **Pas de don** : le système de dons mensuels a été retiré — il créait
  une confusion avec les abonnements (les perks étaient déjà inclus dans
  Plus/Pro) et ne générait aucun revenu (0 don en prod).
- Pas de plan annuel pour l'instant : valider la rétention mensuelle
  d'abord. Réévaluer à 50+ abonnés (offre -20 % annuel).
- Les pubs AdSense restent sur le gratuit : c'est le coût acceptable de
  la monétisation indirecte des non-payants — et un argument Plus concret.

## Remboursements & droit de rétractation (B2C)

- **Base légale** : art. L221-18 C. conso — 14 jours de rétractation pour
  le consommateur. Pour du contenu numérique exécuté immédiatement, le
  droit peut être levé par une **renonciation expresse** (art. L221-28
  13°) : c'est la case à cocher des pages `/premium` et `/docs/support`,
  dont le libellé est repris dans les CGV (art. 6). Les boutons Stripe
  restent verrouillés tant que la case n'est pas cochée.
- **Révocation automatique** : un remboursement **intégral** dans le
  dashboard Stripe déclenche `charge.refunded` → la souscription passe à
  `canceled`, les perks (ads, revive, tag, venue Pro) sont retirés et
  l'utilisateur est notifié par e-mail. La résolution se fait par
  `stripe_invoice_id` (maintenu via `invoice.payment_succeeded`) avec
  fallback sur le customer Stripe si un seul abonnement est actif.
- **Remboursement partiel** : ignoré volontairement — un geste commercial
  ne doit pas couper l'abonnement.
- **Procédure ops** : remboursement dans Stripe (Payments → la charge →
  Refund) **puis** résiliation de l'abonnement dans le dashboard (ou
  laisser le portail client le faire). Le webhook `charge.refunded` couvre
  le cas où seul le refund est fait ; `customer.subscription.deleted`
  couvre la résiliation sans refund.
- **Fait côté Stripe (prod, via API)** : webhook endpoint
  `/api/v1/stripe/webhook` écoute les 7 events (`checkout.session.completed`,
  `async_payment_succeeded`, `customer.subscription.*`,
  `invoice.payment_succeeded`, `charge.refunded`) ; les deux Payment Links
  Plus/Pro ont `consent_collection.terms_of_service=required` ; le Payment
  Link de don one-time est désactivé.
- **Reste à vérifier (dashboard, manuel)** : Settings → Public details →
  *Terms of service URL* doit pointer vers
  `/pages/conditions-generales-vente` — c'est la cible du checkbox CGV
  affiché par Stripe au paiement.

## KPIs à suivre (dashboard admin ou requêtes SQL)

| Métrique | Cible 90 jours | Source |
|---|---|---|
| Utilisateurs actifs hebdo | 500+ | rooms/parties |
| Taux free → inscription | > 30 % | auth |
| Taux inscription → payant | > 2 % | user_subscriptions |
| MRR | > 300 € | user_subscriptions actives |
| Bars Pro | 3-5 | rooms.is_pro |
| Churn mensuel | < 15 % | subscription.deleted events |

Requête MRR rapide :

```sql
SELECT plan, COUNT(*) AS subs,
       COUNT(*) * CASE plan WHEN 'pro' THEN 29.00 ELSE 4.99 END AS mrr_eur
FROM user_subscriptions
WHERE status IN ('active','trialing','past_due')
GROUP BY plan;
```

## Roadmap 90 jours

- **Sem. 1-2** : déployer la bascule dons → abonnements (cette branche),
  vérifier le tunnel Stripe en conditions réelles (1 paiement test).
- **Sem. 2-4** : outreach bars (30 contacts), soumission stores Android,
  4 posts courts.
- **Mois 2** : itérer sur les objections (pricing, features manquantes),
  contenu SEO (2-3 pages), iOS si Android validé.
- **Mois 3** : décision annuel, parrainage, ou feature Pro n°2
  (sonorisation/quiz house depuis la room ?).

## Ce qui n'est PAS dans le scope
- Reconstruction du billing : Stripe Payment Links + webhooks +
  Customer Portal suffisent à cette échelle.
- Programme d'affiliation, facturation entreprise, multi-devises :
  prématuré avant 100+ abonnés.
- Marketing payant (Ads, influenceurs) : burn sans produit-prouvé.
