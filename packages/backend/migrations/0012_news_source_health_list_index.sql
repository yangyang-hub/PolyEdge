CREATE INDEX IF NOT EXISTS news_source_health_source_type_updated_at_idx
  ON news_source_health (source_type, updated_at DESC, source);
