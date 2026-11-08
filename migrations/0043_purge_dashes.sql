-- Final em-dash/en-dash sweep on admin-authored copy (same scope as 0030):
-- CMS pages, FAQs and room descriptions. Source files were cleaned in the
-- same commit; this normalizes rows written before the cleanup.

UPDATE pages SET content = replace(replace(replace(content, ' — ', ', '), '—', '-'), '–', '-')
WHERE content LIKE '%—%' OR content LIKE '%–%';

UPDATE pages SET title = replace(replace(replace(title, ' — ', ', '), '—', '-'), '–', '-')
WHERE title LIKE '%—%' OR title LIKE '%–%';

UPDATE faqs SET question = replace(replace(replace(question, ' — ', ', '), '—', '-'), '–', '-')
WHERE question LIKE '%—%' OR question LIKE '%–%';

UPDATE faqs SET answer = replace(replace(replace(answer, ' — ', ', '), '—', '-'), '–', '-')
WHERE answer LIKE '%—%' OR answer LIKE '%–%';

UPDATE rooms SET description = replace(replace(replace(description, ' — ', ', '), '—', '-'), '–', '-')
WHERE description LIKE '%—%' OR description LIKE '%–%';
