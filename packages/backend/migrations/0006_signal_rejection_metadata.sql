ALTER TABLE signals
    ADD COLUMN rejected_by_user_id TEXT,
    ADD COLUMN rejected_at TIMESTAMPTZ;
