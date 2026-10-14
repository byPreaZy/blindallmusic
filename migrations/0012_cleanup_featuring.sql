-- Remove junk "Featuring" answers imported from compilation metadata:
-- "Disney", "Chorus - Mulan", "Cast - Tarzan", "Ensemble …" etc. are not
-- real featured artists and inflate the answer board with untypeable rows.

DELETE FROM track_answers ta
USING answer_types at
WHERE ta.answer_type_id = at.id
  AND at.name = 'Featuring'
  AND ta.value ~* '^(disney|chorus|ch[œoe]urs?|cast|ensemble|various|singers?|voices|compan(y|ie)|studios?|full cast|bande originale|the cast)([[:>:]].*)?$';
