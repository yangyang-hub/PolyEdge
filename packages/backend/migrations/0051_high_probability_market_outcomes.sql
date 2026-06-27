CREATE TABLE high_probability_market_outcomes (
    condition_id TEXT PRIMARY KEY,
    status TEXT NOT NULL
        CHECK (status IN ('unresolved', 'resolved', 'voided', 'ambiguous')),
    winning_token_id TEXT,
    resolved_at TIMESTAMPTZ,
    market_type TEXT NOT NULL DEFAULT 'unknown',
    risk_tags JSONB NOT NULL DEFAULT '[]',
    label_source TEXT NOT NULL DEFAULT 'manual',
    raw JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        status <> 'resolved'
        OR (winning_token_id IS NOT NULL AND winning_token_id <> '' AND resolved_at IS NOT NULL)
    )
);

CREATE INDEX high_probability_market_outcomes_status_idx
    ON high_probability_market_outcomes (status, resolved_at DESC);
