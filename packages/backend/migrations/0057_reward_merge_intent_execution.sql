ALTER TABLE reward_merge_intents
    ADD COLUMN tx_hash TEXT,
    ADD COLUMN submitted_at TIMESTAMPTZ,
    ADD COLUMN confirmed_at TIMESTAMPTZ,
    ADD COLUMN failed_reason TEXT,
    ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;

CREATE INDEX reward_merge_intents_executable_idx
    ON reward_merge_intents (account_id, status, updated_at ASC)
    WHERE status IN ('pending', 'unsupported');
