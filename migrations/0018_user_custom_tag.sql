-- Per-user display tag shown next to the name in rooms/chat.
ALTER TABLE users ADD COLUMN IF NOT EXISTS custom_tag varchar(24);
