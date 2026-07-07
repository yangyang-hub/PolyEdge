CREATE TABLE reward_fair_values (
    condition_id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    fair_yes NUMERIC NOT NULL,
    fair_no NUMERIC NOT NULL,
    market_midpoint_yes NUMERIC,
    confidence NUMERIC NOT NULL,
    uncertainty_cents NUMERIC NOT NULL,
    midpoint_deviation_cents NUMERIC,
    sample_count BIGINT NOT NULL DEFAULT 0,
    components_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    do_not_quote_reason TEXT,
    observed_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE reward_fair_value_history (
    id UUID PRIMARY KEY,
    condition_id TEXT NOT NULL,
    source TEXT NOT NULL,
    fair_yes NUMERIC NOT NULL,
    fair_no NUMERIC NOT NULL,
    market_midpoint_yes NUMERIC,
    confidence NUMERIC NOT NULL,
    uncertainty_cents NUMERIC NOT NULL,
    midpoint_deviation_cents NUMERIC,
    sample_count BIGINT NOT NULL DEFAULT 0,
    components_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    do_not_quote_reason TEXT,
    observed_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reward_fair_value_history_condition_observed_idx
    ON reward_fair_value_history (condition_id, observed_at DESC);

CREATE INDEX reward_fair_value_history_created_idx
    ON reward_fair_value_history (created_at DESC);
