CREATE TABLE reward_low_competition_observations (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    market_slug TEXT NOT NULL,
    question TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('observe', 'enforce')),
    planned_notional_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    qualified_competition_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    estimated_reward_per_100_usd_day NUMERIC(20,8) NOT NULL DEFAULT 0,
    competition_density NUMERIC(20,8) NOT NULL DEFAULT 0,
    exit_depth_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    exit_slippage_cents NUMERIC(20,8),
    midpoint_range_cents NUMERIC(20,8),
    top_of_book_flip_count BIGINT,
    sample_count BIGINT NOT NULL DEFAULT 0,
    sample_insufficient BOOLEAN NOT NULL DEFAULT false,
    eligible_for_low_competition BOOLEAN NOT NULL DEFAULT false,
    final_eligible BOOLEAN NOT NULL DEFAULT false,
    ai_blocked BOOLEAN NOT NULL DEFAULT false,
    info_risk_blocked BOOLEAN NOT NULL DEFAULT false,
    standard_plan_overlap BOOLEAN NOT NULL DEFAULT false,
    rejection_reasons JSONB NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(rejection_reasons) = 'array'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reward_low_competition_obs_recent_idx
    ON reward_low_competition_observations (account_id, observed_at DESC);

CREATE INDEX reward_low_competition_obs_condition_recent_idx
    ON reward_low_competition_observations (condition_id, observed_at DESC);
