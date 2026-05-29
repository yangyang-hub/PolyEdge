ALTER TABLE markets ADD COLUMN slug TEXT;
CREATE INDEX markets_slug_idx ON markets (slug) WHERE slug IS NOT NULL;
