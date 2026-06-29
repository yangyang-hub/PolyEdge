ALTER TABLE reward_managed_orders
    ADD COLUMN strategy_profile TEXT NOT NULL DEFAULT 'standard'
        CHECK (strategy_profile IN ('standard', 'balanced_merge'));

CREATE INDEX reward_managed_orders_profile_status_idx
    ON reward_managed_orders (strategy_profile, account_id, status, updated_at DESC);

ALTER TABLE reward_quote_plans
    ADD COLUMN strategy_profile TEXT NOT NULL DEFAULT 'standard'
        CHECK (strategy_profile IN ('standard', 'balanced_merge'));

CREATE INDEX reward_quote_plans_profile_eligible_score_idx
    ON reward_quote_plans (strategy_profile, eligible, score DESC, updated_at DESC);

CREATE TABLE reward_merge_intents (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    yes_token_id TEXT NOT NULL,
    no_token_id TEXT NOT NULL,
    merge_size NUMERIC(24, 8) NOT NULL CHECK (merge_size > 0),
    yes_position_size NUMERIC(24, 8) NOT NULL CHECK (yes_position_size >= 0),
    no_position_size NUMERIC(24, 8) NOT NULL CHECK (no_position_size >= 0),
    yes_avg_price NUMERIC(12, 6) NOT NULL DEFAULT 0,
    no_avg_price NUMERIC(12, 6) NOT NULL DEFAULT 0,
    status TEXT NOT NULL CHECK (
        status IN ('pending', 'unsupported', 'submitted', 'completed', 'failed')
    ),
    reason TEXT NOT NULL,
    source_fill_id TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX reward_merge_intents_source_fill_idx
    ON reward_merge_intents (source_fill_id);

CREATE INDEX reward_merge_intents_account_condition_status_idx
    ON reward_merge_intents (account_id, condition_id, status, updated_at DESC);
