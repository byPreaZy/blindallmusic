-- FR/EN aliases for media answers so both language variants validate.
-- Bidirectional: each pair is applied in both directions.

WITH pairs(pair_en, pair_fr) AS (
    VALUES
        ('Hercules', 'Hercule'),
        ('The Lion King', 'Le Roi Lion'),
        ('The Lion King 2', 'Le Roi Lion 2'),
        ('The Lion King 3', 'Le Roi Lion 3'),
        ('Beauty and the Beast', 'La Belle et la Bête'),
        ('Cinderella', 'Cendrillon'),
        ('Snow White', 'Blanche-Neige'),
        ('Snow White and the Seven Dwarfs', 'Blanche-Neige et les Sept Nains'),
        ('The Little Mermaid', 'La Petite Sirène'),
        ('Frozen', 'La Reine des Neiges'),
        ('Frozen 2', 'La Reine des Neiges 2'),
        ('Frozen II', 'La Reine des Neiges 2'),
        ('Moana', 'Vaiana'),
        ('Tangled', 'Raiponce'),
        ('Brave', 'Rebelle'),
        ('The Jungle Book', 'Le Livre de la Jungle'),
        ('Finding Nemo', 'Le Monde de Nemo'),
        ('Finding Dory', 'Le Monde de Dory'),
        ('Monsters, Inc.', 'Monstres et Cie'),
        ('Monsters Inc', 'Monstres et Cie'),
        ('The Incredibles', 'Les Indestructibles'),
        ('Incredibles 2', 'Les Indestructibles 2'),
        ('Wreck-It Ralph', 'Les Mondes de Ralph'),
        ('Ralph Breaks the Internet', 'Ralph 2.0'),
        ('Zootopia', 'Zootopie'),
        ('Inside Out', 'Vice-Versa'),
        ('Inside Out 2', 'Vice-Versa 2'),
        ('Sleeping Beauty', 'La Belle au Bois Dormant'),
        ('Lady and the Tramp', 'La Belle et le Clochard'),
        ('The Aristocats', 'Les Aristochats'),
        ('The Hunchback of Notre Dame', 'Le Bossu de Notre-Dame'),
        ('Brother Bear', 'Frère des Ours'),
        ('Atlantis: The Lost Empire', 'Atlantide, l''empire perdu'),
        ('Treasure Planet', 'La Planète au Trésor'),
        ('Up', 'Là-haut'),
        ('A Bug''s Life', '1001 Pattes'),
        ('The Emperor''s New Groove', 'Kuzco, l''empereur mégalo'),
        ('The Princess and the Frog', 'La Princesse et la Grenouille'),
        ('Big Hero 6', 'Les Nouveaux Héros'),
        ('Turning Red', 'Alerte Rouge'),
        ('Elemental', 'Élémentaire'),
        ('The Fox and the Hound', 'Rox et Rouky'),
        ('Oliver & Company', 'Oliver et Compagnie'),
        ('A Goofy Movie', 'Dingo et Max'),
        ('Wish', 'Wish : Asha et la bonne étoile'),
        ('Encanto', 'Encanto: La fantastique famille Madrigal'),
        ('Winnie the Pooh', 'Les Aventures de Winnie l''Ourson'),
        ('Pocahontas', 'Pocahontas - Une Légende Indienne'),
        ('Cars', 'Cars : Quatre Roues'),
        ('Lilo & Stitch', 'Lilo et Stitch'),
        ('The Many Adventures of Winnie the Pooh', 'Les Aventures de Winnie l''Ourson')
)
UPDATE track_answers ta
SET aliases = (
        SELECT to_jsonb(array_agg(DISTINCT a))
        FROM (
            SELECT jsonb_array_elements_text(coalesce(ta.aliases, '[]'::jsonb)) AS a
            UNION
            SELECT p.pair_fr FROM pairs p WHERE lower(ta.value) = lower(p.pair_en)
            UNION
            SELECT p.pair_en FROM pairs p WHERE lower(ta.value) = lower(p.pair_fr)
        ) merged
        WHERE a <> '' AND lower(a) <> lower(ta.value)
    ),
    updated_at = now()
FROM (
    SELECT pair_en AS v FROM pairs
    UNION
    SELECT pair_fr FROM pairs
) all_vals
WHERE lower(ta.value) = lower(all_vals.v);
