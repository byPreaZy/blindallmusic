-- Seasonal visibility for event rooms. NULL = visible year-round;
-- otherwise the room only appears in public discovery listings during
-- those months (direct slug links keep working).
ALTER TABLE rooms ADD COLUMN IF NOT EXISTS visible_months smallint[];

UPDATE rooms SET visible_months = '{2,3}'   WHERE slug = 'saint-patrick';
UPDATE rooms SET visible_months = '{6,7,8}' WHERE slug = 'summer';
UPDATE rooms SET visible_months = '{9,10}'  WHERE slug = 'halloween';
UPDATE rooms SET visible_months = '{12,1}'  WHERE slug = 'nouvel-an-2026';
UPDATE rooms SET visible_months = '{11,12}' WHERE slug = 'cest-noel';
UPDATE rooms SET visible_months = '{1,2}'   WHERE slug = 'love';
UPDATE rooms SET visible_months = '{5,6}'   WHERE slug = 'eurovision';
