-- Optional venue display name shown on the salon/TV view (B2B host branding).
ALTER TABLE rooms ADD COLUMN IF NOT EXISTS venue_name text;
