pub struct CategoryContent {
    pub title: String,
    pub meta_title: String,
    pub meta_description: String,
    pub heading: String,
    pub intro: String,
    pub intro_secondary: String,
    pub rooms_heading: String,
    pub faq_title: String,
    pub faq: Vec<(String, String)>,
}

fn tpl<'a>(generic: &'a str, slug: &'a str, field: &str) -> &'a str {
    slug_override(slug, field).unwrap_or(generic)
}

fn slug_override(slug: &str, field: &str) -> Option<&'static str> {
    let v = match (slug, field) {
        ("genre-musical", "faq_a1") => {
            "Les salles officielles couvrent notamment le Rap FR, le Rock, la Pop, la Chanson française, l'Electro, le Jazz, le K-pop, le Métal, la Musique classique, le Rap & RnB et un Quiz Général multi-genres."
        }
        ("genre-musical", "faq_a2") => {
            "Choisis une salle ci-dessus, rejoins la partie et écoute l'extrait. Tape le titre, l'artiste ou la réponse demandée le plus vite possible pour marquer des points."
        }
        ("genre-musical", "faq_a3") => {
            "Non. Toutes les salles officielles Blind'All Music sont gratuites et jouables directement dans ton navigateur, sur ordinateur ou mobile."
        }
        ("genre-musical", "faq_q1") => {
            "Quels styles musicaux sont disponibles sur Blind'All Music ?"
        }
        ("genre-musical", "faq_q2") => "Comment jouer à un blind test musical ?",
        ("genre-musical", "faq_q3") => "Faut-il payer ou installer une application ?",
        ("genre-musical", "heading") => "Blind test par genre musical",
        ("genre-musical", "intro") => {
            "Sur Blind'All Music, retrouve :count salles officielles dédiées aux grands styles musicaux : :rooms. Chaque room propose des extraits sélectionnés pour tester ta culture musicale en multijoueur, en temps réel."
        }
        ("genre-musical", "intro_secondary") => {
            "Que tu sois fan de variété française, de tubes internationaux ou de morceaux plus pointus, choisis la salle qui correspond à ton style et affronte d'autres joueurs. Gratuit, sans téléchargement, directement dans le navigateur."
        }
        ("genre-musical", "meta_description") => {
            "Joue gratuitement à des blind tests par style musical sur Blind'All Music : Rap FR, Rock, Pop, Chanson française, Electro, Jazz, K-pop, Métal et plus. Multijoueur en direct."
        }
        ("genre-musical", "meta_title") => {
            "Blind test par genre musical, Rap, Rock, Pop, Jazz | Blind'All Music"
        }
        ("genre-musical", "rooms_heading") => "Salles officielles par genre musical",
        ("genre-musical", "schema_name") => "Blind tests par genre musical sur Blind'All Music",
        ("les-decennies", "faq_a1") => {
            "Tu peux jouer sur les Années 70, 80, 90, 2000, 2010, 2020, ainsi qu'une salle Time Machine pour les titres d'avant les années 70."
        }
        ("les-decennies", "faq_a2") => {
            "Oui. Les salles sont multijoueur : plusieurs personnes peuvent rejoindre la même partie et répondre en même temps aux extraits."
        }
        ("les-decennies", "faq_a3") => {
            "Les modérateurs Blind'All Music enrichissent régulièrement les playlists officielles pour proposer un bon équilibre entre tubes cultes et découvertes."
        }
        ("les-decennies", "faq_q1") => "Quelles décennies sont disponibles ?",
        ("les-decennies", "faq_q2") => "Ces blind tests conviennent-ils à un groupe ?",
        ("les-decennies", "faq_q3") => "Les playlists sont-elles mises à jour ?",
        ("les-decennies", "heading") => "Blind test par décennie",
        ("les-decennies", "intro") => {
            "Replonge dans les tubes qui ont marqué chaque époque avec :count salles officielles : :rooms. Des années 70 aux hits actuels, teste ta mémoire musicale décennie par décennie."
        }
        ("les-decennies", "intro_secondary") => {
            "Chaque salle est dédiée à une période précise, avec des playlists pensées pour retrouver les classiques, les one-hit wonders et les incontournables du moment. Idéal pour les soirées nostalgie entre amis."
        }
        ("les-decennies", "meta_description") => {
            "Blind tests gratuits par décennie sur Blind'All Music : années 70, 80, 90, 2000, 2010, 2020 et plus. Retrouve les tubes qui ont marqué chaque époque."
        }
        ("les-decennies", "meta_title") => {
            "Blind test par décennie : années 70, 80, 90, 2000 | Blind'All Music"
        }
        ("les-decennies", "rooms_heading") => "Salles officielles par décennie",
        ("les-decennies", "schema_name") => "Blind tests par décennie sur Blind'All Music",
        ("les-speciales", "faq_a1") => {
            "Certaines salles sont permanentes (comme Love), d'autres sont liées à une saison ou une fête : Noël, Halloween, Saint Patrick, Oktoberfest, été, etc."
        }
        ("les-speciales", "faq_a2") => {
            "Oui, c'est l'usage principal : lancer une room thématique et jouer à plusieurs sur les mêmes extraits, en direct."
        }
        ("les-speciales", "faq_a3") => {
            "Comme toutes les salles officielles Blind'All Music, elles sont gratuites et accessibles depuis le navigateur."
        }
        ("les-speciales", "faq_q1") => "Quand les salles spéciales sont-elles disponibles ?",
        ("les-speciales", "faq_q2") => "Est-ce adapté pour une soirée entre amis ?",
        ("les-speciales", "faq_q3") => "Les salles spéciales sont-elles gratuites ?",
        ("les-speciales", "heading") => "Blind tests événementiels et spéciaux",
        ("les-speciales", "intro") => {
            "Découvre :count salles officielles pour les occasions et fêtes : :rooms. Des playlists pensées pour animer une soirée, un apéro ou un événement à thème."
        }
        ("les-speciales", "intro_secondary") => {
            "Ces éditions spéciales reviennent selon le calendrier et les événements. Rejoins la communauté Blind'All Music pour profiter de blind tests festifs en multijoueur."
        }
        ("les-speciales", "meta_description") => {
            "Blind tests thématiques et événementiels sur Blind'All Music : Noël, Halloween, Saint Patrick, Oktoberfest, Love et éditions saisonnières."
        }
        ("les-speciales", "meta_title") => {
            "Blind test Noël, Halloween, été, éditions spéciales | Blind'All Music"
        }
        ("les-speciales", "rooms_heading") => "Salles officielles événementielles",
        ("les-speciales", "schema_name") => "Blind tests événementiels sur Blind'All Music",
        ("sur-les-ecrans", "faq_a1") => {
            "Les salles incluent notamment Disney, les films, les séries TV, l'anime/manga, les dessins animés, les jeux vidéo, l'Eurovision, les pubs TV, TikTok Hits et les youtubeurs."
        }
        ("sur-les-ecrans", "faq_a2") => {
            "Pas forcément. Selon la salle, il faut retrouver le titre, l'artiste, l'œuvre ou parfois un indice lié au morceau. La rapidité compte autant que la justesse."
        }
        ("sur-les-ecrans", "faq_a3") => {
            "Oui. Partage le lien de la salle avec tes amis pour jouer ensemble au même blind test en direct."
        }
        ("sur-les-ecrans", "faq_q1") => "Quels univers sont couverts ?",
        ("sur-les-ecrans", "faq_q2") => "Dois-je connaître les paroles pour gagner ?",
        ("sur-les-ecrans", "faq_q3") => "Puis-je jouer avec des amis ?",
        ("sur-les-ecrans", "heading") => "Blind test cinéma, séries et culture pop",
        ("sur-les-ecrans", "intro") => {
            "Retrouve :count salles officielles autour des musiques des écrans et de la culture pop : :rooms. Bandes originales, génériques, tubes de films, séries, jeux vidéo et phénomènes internet."
        }
        ("sur-les-ecrans", "intro_secondary") => {
            "Parfait pour les fans de Disney, d'anime, de séries cultes ou de jeux vidéo. Rejoins une partie en cours ou lance une manche quand tu veux, gratuitement, sans inscription obligatoire."
        }
        ("sur-les-ecrans", "meta_description") => {
            "Blind tests gratuits sur la culture pop : Disney, films, séries TV, anime, jeux vidéo, Eurovision, pubs TV et plus. Multijoueur sur Blind'All Music."
        }
        ("sur-les-ecrans", "meta_title") => {
            "Blind test Disney, films, séries, jeux vidéo | Blind'All Music"
        }
        ("sur-les-ecrans", "rooms_heading") => "Salles officielles culture pop",
        ("sur-les-ecrans", "schema_name") => {
            "Blind tests culture pop et écrans sur Blind'All Music"
        }
        _ => return None,
    };
    Some(v)
}

fn fill(t: &str, category: &str, count: i64, rooms: &str) -> String {
    t.replace(":category", category)
        .replace(":count", &count.to_string())
        .replace(":rooms", rooms)
}

pub fn content(slug: &str, category: &str, count: i64, rooms: &str) -> CategoryContent {
    let title = fill(
        tpl(
            "Blind test :category, quiz musical multijoueur gratuit | Blind'All Music",
            slug,
            "title",
        ),
        category,
        count,
        rooms,
    );
    let meta_title = fill(
        tpl(
            "Blind test :category, quiz musical multijoueur gratuit | Blind'All Music",
            slug,
            "meta_title",
        ),
        category,
        count,
        rooms,
    );
    let meta_description = fill(
        tpl(
            "Joue gratuitement à des blind tests et quiz musicaux :category sur Blind'All Music. Salles officielles multijoueur en direct, sans téléchargement.",
            slug,
            "meta_description",
        ),
        category,
        count,
        rooms,
    );
    let heading = fill(
        tpl("Blind test :category en ligne", slug, "heading"),
        category,
        count,
        rooms,
    );
    let intro = fill(
        tpl(
            "Découvre :count salles officielles de blind test :category sur Blind'All Music. Écoute l'extrait, trouve le titre ou l'artiste et affronte d'autres joueurs en temps réel, gratuitement dans ton navigateur.",
            slug,
            "intro",
        ),
        category,
        count,
        rooms,
    );
    let intro_secondary = fill(
        tpl(
            "Les rooms :category sont animées par l'équipe Blind'All Music et la communauté. Rejoins une partie en cours ou lance une nouvelle manche quand tu veux.",
            slug,
            "intro_secondary",
        ),
        category,
        count,
        rooms,
    );
    let rooms_heading = fill(
        tpl("Salles officielles :category", slug, "rooms_heading"),
        category,
        count,
        rooms,
    );
    let faq_title = fill(
        tpl(
            "Questions fréquentes : blind test :category",
            slug,
            "faq_title",
        ),
        category,
        count,
        rooms,
    );
    let faq = vec![
        (
            fill(
                tpl(
                    "Comment jouer à un blind test :category sur Blind'All Music ?",
                    slug,
                    "faq_q1",
                ),
                category,
                count,
                rooms,
            ),
            fill(
                tpl(
                    "Choisis une salle :category ci-dessus, rejoins la partie et écoute les extraits. Tape ta réponse le plus vite possible pour marquer des points.",
                    slug,
                    "faq_a1",
                ),
                category,
                count,
                rooms,
            ),
        ),
        (
            fill(
                tpl("Faut-il créer un compte pour jouer ?", slug, "faq_q2"),
                category,
                count,
                rooms,
            ),
            fill(
                tpl(
                    "Tu peux rejoindre une room en invité, mais un compte gratuit te permet de sauvegarder ton score, ton niveau et ton ELO.",
                    slug,
                    "faq_a2",
                ),
                category,
                count,
                rooms,
            ),
        ),
        (
            fill(
                tpl(
                    "Les blind tests :category sont-ils gratuits ?",
                    slug,
                    "faq_q3",
                ),
                category,
                count,
                rooms,
            ),
            fill(
                tpl(
                    "Oui, toutes les salles officielles Blind'All Music sont gratuites et jouables directement dans le navigateur.",
                    slug,
                    "faq_a3",
                ),
                category,
                count,
                rooms,
            ),
        ),
    ];
    CategoryContent {
        title,
        meta_title,
        meta_description,
        heading,
        intro,
        intro_secondary,
        rooms_heading,
        faq_title,
        faq,
    }
}
