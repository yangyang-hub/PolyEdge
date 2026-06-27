CREATE TABLE high_probability_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE high_probability_samples (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    sampled_at TIMESTAMPTZ NOT NULL,
    trigger_kind TEXT NOT NULL
        CHECK (trigger_kind IN ('first_touch', 'sustained', 're_entry')),
    executable_price NUMERIC NOT NULL CHECK (executable_price >= 0 AND executable_price <= 1),
    price_bucket TEXT NOT NULL,
    market_type TEXT NOT NULL,
    time_to_resolution_bucket TEXT,
    liquidity_bucket TEXT,
    spread_bucket TEXT,
    path_features JSONB NOT NULL DEFAULT '{}',
    risk_tags JSONB NOT NULL DEFAULT '[]',
    outcome TEXT NOT NULL DEFAULT 'unknown'
        CHECK (outcome IN ('win', 'loss', 'voided', 'unknown')),
    settlement_pnl NUMERIC,
    max_drawdown_cents NUMERIC,
    hold_seconds BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (condition_id, token_id, sampled_at, trigger_kind, price_bucket)
);

CREATE INDEX high_probability_samples_bucket_idx
    ON high_probability_samples (market_type, price_bucket, sampled_at DESC);
CREATE INDEX high_probability_samples_condition_idx
    ON high_probability_samples (condition_id);
CREATE INDEX high_probability_samples_outcome_idx
    ON high_probability_samples (outcome, sampled_at DESC);

CREATE TABLE high_probability_bucket_stats (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    model_version TEXT NOT NULL,
    bucket_key TEXT NOT NULL,
    bucket_dimensions JSONB NOT NULL,
    sample_count BIGINT NOT NULL CHECK (sample_count >= 0),
    win_count BIGINT NOT NULL CHECK (win_count >= 0),
    win_rate NUMERIC NOT NULL CHECK (win_rate >= 0 AND win_rate <= 1),
    fair_probability NUMERIC NOT NULL CHECK (fair_probability >= 0 AND fair_probability <= 1),
    confidence_low NUMERIC CHECK (confidence_low >= 0 AND confidence_low <= 1),
    confidence_high NUMERIC CHECK (confidence_high >= 0 AND confidence_high <= 1),
    expected_pnl NUMERIC,
    avg_max_drawdown_cents NUMERIC,
    break_70_rate NUMERIC CHECK (break_70_rate >= 0 AND break_70_rate <= 1),
    break_60_rate NUMERIC CHECK (break_60_rate >= 0 AND break_60_rate <= 1),
    break_50_rate NUMERIC CHECK (break_50_rate >= 0 AND break_50_rate <= 1),
    avg_hold_seconds BIGINT,
    recommended_max_entry_price NUMERIC
        CHECK (recommended_max_entry_price >= 0 AND recommended_max_entry_price <= 1),
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (model_version, bucket_key)
);

CREATE INDEX high_probability_bucket_stats_model_idx
    ON high_probability_bucket_stats (model_version, sample_count DESC);

CREATE TABLE high_probability_observations (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    observed_at TIMESTAMPTZ NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('observe', 'paper', 'live_guarded')),
    executable_price NUMERIC NOT NULL CHECK (executable_price >= 0 AND executable_price <= 1),
    fair_probability NUMERIC CHECK (fair_probability >= 0 AND fair_probability <= 1),
    net_edge NUMERIC,
    recommended_size_usd NUMERIC,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'reject', 'skip')),
    reasons JSONB NOT NULL DEFAULT '[]',
    model_version TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX high_probability_observations_recent_idx
    ON high_probability_observations (observed_at DESC);
CREATE INDEX high_probability_observations_condition_idx
    ON high_probability_observations (condition_id, observed_at DESC);
