CREATE TABLE reward_bot_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE reward_markets (
    condition_id TEXT PRIMARY KEY,
    question TEXT NOT NULL,
    market_slug TEXT NOT NULL,
    event_slug TEXT NOT NULL DEFAULT '',
    image TEXT NOT NULL DEFAULT '',
    rewards_max_spread NUMERIC(12, 6) NOT NULL CHECK (rewards_max_spread >= 0),
    rewards_min_size NUMERIC(24, 8) NOT NULL CHECK (rewards_min_size >= 0),
    total_daily_rate NUMERIC(14, 4) NOT NULL CHECK (total_daily_rate >= 0),
    tokens_json JSONB NOT NULL CHECK (jsonb_typeof(tokens_json) = 'array'),
    active BOOLEAN NOT NULL DEFAULT true,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX reward_markets_active_rate_idx
    ON reward_markets (active, total_daily_rate DESC, updated_at DESC);

CREATE TABLE reward_quote_plans (
    condition_id TEXT PRIMARY KEY REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    score NUMERIC(10, 4) NOT NULL CHECK (score >= 0),
    eligible BOOLEAN NOT NULL DEFAULT false,
    reason TEXT NOT NULL,
    quote_plan_json JSONB NOT NULL CHECK (jsonb_typeof(quote_plan_json) = 'object'),
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX reward_quote_plans_eligible_score_idx
    ON reward_quote_plans (eligible, score DESC, updated_at DESC);

CREATE TABLE reward_managed_orders (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 6) NOT NULL CHECK (price > 0 AND price < 1),
    size NUMERIC(24, 8) NOT NULL CHECK (size > 0),
    external_order_id TEXT,
    status TEXT NOT NULL CHECK (
        status IN ('planned', 'open', 'cancelled', 'filled', 'exit_pending', 'error')
    ),
    scoring BOOLEAN NOT NULL DEFAULT false,
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL
);

CREATE UNIQUE INDEX reward_managed_orders_external_order_id_idx
    ON reward_managed_orders (external_order_id)
    WHERE external_order_id IS NOT NULL;

CREATE INDEX reward_managed_orders_account_status_idx
    ON reward_managed_orders (account_id, status, updated_at DESC);

CREATE INDEX reward_managed_orders_condition_status_idx
    ON reward_managed_orders (condition_id, status, updated_at DESC);

CREATE TABLE reward_positions (
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    size NUMERIC(24, 8) NOT NULL DEFAULT 0,
    avg_price NUMERIC(12, 6) NOT NULL DEFAULT 0,
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (account_id, token_id)
);

CREATE INDEX reward_positions_account_condition_idx
    ON reward_positions (account_id, condition_id);

CREATE TABLE reward_risk_events (
    id TEXT PRIMARY KEY,
    account_id TEXT,
    condition_id TEXT,
    external_order_id TEXT,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    message TEXT NOT NULL,
    metadata_json JSONB NOT NULL CHECK (jsonb_typeof(metadata_json) = 'object'),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX reward_risk_events_created_at_idx
    ON reward_risk_events (created_at DESC);

CREATE INDEX reward_risk_events_account_created_at_idx
    ON reward_risk_events (account_id, created_at DESC);
