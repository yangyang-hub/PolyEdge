-- Fair value snapshots produced by the high-probability pricing model provider.
-- One row per (condition_id, model_version): the latest estimate replaces prior
-- rows. The Rewards market maker reads only non-expired rows for the enabled
-- model version. See doc/high-probability-pricing-strategy-plan.md (Phase 3).
CREATE TABLE reward_market_fair_values (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    side_used TEXT NOT NULL CHECK (side_used IN ('yes', 'no_complement')),
    price_used NUMERIC NOT NULL CHECK (price_used >= 0 AND price_used <= 1),
    fair_yes_low NUMERIC NOT NULL CHECK (fair_yes_low >= 0 AND fair_yes_low <= 1),
    fair_yes_mid NUMERIC NOT NULL CHECK (fair_yes_mid >= 0 AND fair_yes_mid <= 1),
    fair_yes_high NUMERIC NOT NULL CHECK (fair_yes_high >= 0 AND fair_yes_high <= 1),
    CHECK (fair_yes_low <= fair_yes_mid AND fair_yes_mid <= fair_yes_high),
    market_implied NUMERIC NOT NULL CHECK (market_implied >= 0 AND market_implied <= 1),
    base_rate NUMERIC NOT NULL CHECK (base_rate >= 0 AND base_rate <= 1),
    confidence NUMERIC NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    uncertainty_cents NUMERIC NOT NULL CHECK (uncertainty_cents >= 0),
    sample_count BIGINT NOT NULL CHECK (sample_count >= 0),
    bucket_key TEXT NOT NULL,
    -- 0 = exact bucket, increasing as the resolution falls back to coarser
    -- buckets, up to 5 for the sample-weighted global prior.
    fallback_level SMALLINT NOT NULL CHECK (fallback_level BETWEEN 0 AND 5),
    model_version TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    reason_codes JSONB NOT NULL DEFAULT '[]',
    live_eligible BOOLEAN NOT NULL DEFAULT FALSE,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (condition_id, model_version)
);

-- Live read path: latest non-expired estimate for the enabled model version.
CREATE INDEX reward_market_fair_values_live_idx
    ON reward_market_fair_values (model_version, live_eligible, expires_at DESC);

CREATE INDEX reward_market_fair_values_recent_idx
    ON reward_market_fair_values (computed_at DESC);

CREATE INDEX reward_market_fair_values_condition_idx
    ON reward_market_fair_values (condition_id, model_version, computed_at DESC);
