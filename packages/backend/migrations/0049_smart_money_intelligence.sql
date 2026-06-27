-- Smart Money Intelligence foundation: candidate wallets, profiles, scores,
-- source trades, signals, advisory caches and paper execution records.

CREATE TABLE smart_money_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE smart_wallet_candidates (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    source TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'candidate'
        CHECK (status IN ('candidate', 'watch', 'tracked', 'blocked', 'rejected')),
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_analyzed_at TIMESTAMPTZ,
    promoted_at TIMESTAMPTZ,
    rejected_at TIMESTAMPTZ,
    reason TEXT,
    raw JSONB NOT NULL DEFAULT '{}',
    UNIQUE (wallet_address, source)
);

CREATE INDEX smart_wallet_candidates_status_seen_idx
    ON smart_wallet_candidates (status, last_seen_at DESC);
CREATE INDEX smart_wallet_candidates_wallet_idx
    ON smart_wallet_candidates (wallet_address);

CREATE TABLE smart_wallet_profiles (
    wallet_address TEXT PRIMARY KEY,
    trade_count BIGINT NOT NULL DEFAULT 0,
    settled_trade_count BIGINT NOT NULL DEFAULT 0,
    total_volume_usd NUMERIC NOT NULL DEFAULT 0,
    realized_pnl_usd NUMERIC NOT NULL DEFAULT 0,
    roi NUMERIC NOT NULL DEFAULT 0,
    win_rate NUMERIC NOT NULL DEFAULT 0,
    max_drawdown_usd NUMERIC NOT NULL DEFAULT 0,
    avg_trade_usd NUMERIC NOT NULL DEFAULT 0,
    median_trade_usd NUMERIC NOT NULL DEFAULT 0,
    avg_hold_secs BIGINT,
    active_days BIGINT NOT NULL DEFAULT 0,
    markets_traded BIGINT NOT NULL DEFAULT 0,
    category_concentration_score NUMERIC NOT NULL DEFAULT 0,
    market_concentration_score NUMERIC NOT NULL DEFAULT 0,
    low_liquidity_trade_ratio NUMERIC NOT NULL DEFAULT 0,
    stale_copy_window_ratio NUMERIC NOT NULL DEFAULT 0,
    last_trade_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_wallet_profiles_updated_idx
    ON smart_wallet_profiles (updated_at DESC);

CREATE TABLE smart_wallet_scores (
    wallet_address TEXT PRIMARY KEY REFERENCES smart_wallet_profiles(wallet_address) ON DELETE CASCADE,
    total_score NUMERIC NOT NULL,
    profit_score NUMERIC NOT NULL,
    consistency_score NUMERIC NOT NULL,
    risk_score NUMERIC NOT NULL,
    liquidity_score NUMERIC NOT NULL,
    recency_score NUMERIC NOT NULL,
    copyability_score NUMERIC NOT NULL,
    tier TEXT NOT NULL CHECK (tier IN ('blocked', 'candidate', 'watch', 'approved')),
    explanation JSONB NOT NULL DEFAULT '{}',
    scoring_version TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_wallet_scores_tier_score_idx
    ON smart_wallet_scores (tier, total_score DESC);

CREATE TABLE smart_wallet_trades (
    id TEXT PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    source TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    outcome TEXT,
    price NUMERIC NOT NULL CHECK (price >= 0 AND price <= 1),
    size NUMERIC NOT NULL CHECK (size >= 0),
    notional_usd NUMERIC NOT NULL CHECK (notional_usd >= 0),
    tx_hash TEXT,
    source_timestamp TIMESTAMPTZ NOT NULL,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    raw JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX smart_wallet_trades_wallet_time_idx
    ON smart_wallet_trades (wallet_address, source_timestamp DESC);
CREATE INDEX smart_wallet_trades_condition_time_idx
    ON smart_wallet_trades (condition_id, source_timestamp DESC);
CREATE INDEX smart_wallet_trades_discovered_idx
    ON smart_wallet_trades (discovered_at DESC);

CREATE TABLE smart_signals (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    source_trade_id TEXT NOT NULL REFERENCES smart_wallet_trades(id),
    wallet_address TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    source_price NUMERIC NOT NULL CHECK (source_price >= 0 AND source_price <= 1),
    current_price NUMERIC CHECK (current_price >= 0 AND current_price <= 1),
    price_slippage_cents NUMERIC,
    latency_ms BIGINT,
    source_notional_usd NUMERIC NOT NULL DEFAULT 0,
    consensus_wallet_count BIGINT NOT NULL DEFAULT 1,
    score NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'new'
        CHECK (status IN ('new', 'rejected', 'observe', 'paper', 'approval_required', 'live_ready', 'executed', 'expired')),
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_signals_status_created_idx
    ON smart_signals (status, created_at DESC);
CREATE INDEX smart_signals_condition_created_idx
    ON smart_signals (condition_id, created_at DESC);
CREATE INDEX smart_signals_wallet_created_idx
    ON smart_signals (wallet_address, created_at DESC);
CREATE INDEX smart_signals_source_trade_idx
    ON smart_signals (source_trade_id);

CREATE TABLE smart_signal_decisions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    signal_id BIGINT NOT NULL REFERENCES smart_signals(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'observe', 'reject')),
    stage TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('observe', 'paper', 'approval', 'live_guarded')),
    rejection_reason TEXT,
    risk_checks JSONB NOT NULL DEFAULT '{}',
    decided_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_signal_decisions_signal_idx
    ON smart_signal_decisions (signal_id, decided_at DESC);
CREATE INDEX smart_signal_decisions_decision_idx
    ON smart_signal_decisions (decision, decided_at DESC);

CREATE TABLE smart_wallet_advisories (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    provider TEXT NOT NULL,
    request_format TEXT NOT NULL,
    model TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    recommendation TEXT NOT NULL CHECK (recommendation IN ('allow', 'observe', 'reject')),
    confidence NUMERIC NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    risk_tags JSONB NOT NULL DEFAULT '[]',
    summary TEXT NOT NULL DEFAULT '',
    reasons JSONB NOT NULL DEFAULT '[]',
    raw_output JSONB NOT NULL DEFAULT '{}',
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_address, provider, request_format, model, input_hash)
);

CREATE INDEX smart_wallet_advisories_lookup_idx
    ON smart_wallet_advisories (wallet_address, expires_at DESC);

CREATE TABLE smart_signal_advisories (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    signal_id BIGINT NOT NULL REFERENCES smart_signals(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    request_format TEXT NOT NULL,
    model TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    recommendation TEXT NOT NULL CHECK (recommendation IN ('allow', 'observe', 'reject')),
    confidence NUMERIC NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    risk_tags JSONB NOT NULL DEFAULT '[]',
    summary TEXT NOT NULL DEFAULT '',
    reasons JSONB NOT NULL DEFAULT '[]',
    raw_output JSONB NOT NULL DEFAULT '{}',
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (signal_id, provider, request_format, model, input_hash)
);

CREATE INDEX smart_signal_advisories_signal_idx
    ON smart_signal_advisories (signal_id, expires_at DESC);

CREATE TABLE smart_paper_executions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    signal_id BIGINT NOT NULL REFERENCES smart_signals(id),
    account_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    token_id TEXT,
    planned_price NUMERIC NOT NULL,
    filled_price NUMERIC,
    size NUMERIC NOT NULL,
    notional_usd NUMERIC NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('planned', 'filled', 'expired', 'closed')),
    realized_pnl_usd NUMERIC NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_paper_executions_signal_idx
    ON smart_paper_executions (signal_id);
CREATE INDEX smart_paper_executions_account_created_idx
    ON smart_paper_executions (account_id, created_at DESC);
