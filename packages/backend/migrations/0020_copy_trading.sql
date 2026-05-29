-- Smart-money copy trading: tracked wallets + wallet analysis stats, detected
-- source trades, mirrored copy orders/positions, a simulated fund-pool ledger,
-- and an activity/risk event log. Mirrors the reward_* simulation tables.

CREATE TABLE copytrade_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE copytrade_wallets (
    address TEXT PRIMARY KEY,
    label TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL CHECK (status IN ('active', 'paused')),
    sizing_override TEXT,
    max_exposure_override NUMERIC(18, 4),
    trades_window INTEGER NOT NULL DEFAULT 0,
    volume_window_usd NUMERIC(18, 4) NOT NULL DEFAULT 0,
    realized_pnl_window NUMERIC(18, 4) NOT NULL DEFAULT 0,
    win_rate NUMERIC(6, 4) NOT NULL DEFAULT 0,
    roi NUMERIC(12, 4) NOT NULL DEFAULT 0,
    avg_trade_usd NUMERIC(18, 4) NOT NULL DEFAULT 0,
    markets_traded INTEGER NOT NULL DEFAULT 0,
    last_active_at TIMESTAMPTZ,
    last_analyzed_at TIMESTAMPTZ,
    added_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX copytrade_wallets_status_idx
    ON copytrade_wallets (status, updated_at DESC);

-- One detected trade from a tracked wallet. `id` is deterministic over the
-- wallet/tx/token/side/timestamp so re-scans dedupe via ON CONFLICT (id).
CREATE TABLE copytrade_source_trades (
    id TEXT PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL DEFAULT '',
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 6) NOT NULL CHECK (price >= 0),
    size NUMERIC(24, 8) NOT NULL CHECK (size >= 0),
    usd_size NUMERIC(18, 4) NOT NULL CHECK (usd_size >= 0),
    title TEXT NOT NULL DEFAULT '',
    source_tx_hash TEXT NOT NULL DEFAULT '',
    source_timestamp TIMESTAMPTZ NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    copied BOOLEAN NOT NULL DEFAULT false,
    decision_reason TEXT NOT NULL DEFAULT ''
);

CREATE INDEX copytrade_source_trades_observed_idx
    ON copytrade_source_trades (observed_at DESC);

CREATE INDEX copytrade_source_trades_wallet_idx
    ON copytrade_source_trades (wallet_address, source_timestamp DESC);

CREATE TABLE copytrade_orders (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    wallet_address TEXT NOT NULL,
    source_trade_id TEXT NOT NULL DEFAULT '',
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL DEFAULT '',
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 6) NOT NULL CHECK (price > 0 AND price < 1),
    size NUMERIC(24, 8) NOT NULL CHECK (size > 0),
    notional_usd NUMERIC(18, 4) NOT NULL CHECK (notional_usd >= 0),
    external_order_id TEXT,
    status TEXT NOT NULL CHECK (
        status IN ('planned', 'open', 'filled', 'cancelled', 'error')
    ),
    reason TEXT NOT NULL DEFAULT '',
    filled_size NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (filled_size >= 0),
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL DEFAULT ''
);

CREATE UNIQUE INDEX copytrade_orders_external_order_id_idx
    ON copytrade_orders (external_order_id)
    WHERE external_order_id IS NOT NULL;

CREATE INDEX copytrade_orders_account_status_idx
    ON copytrade_orders (account_id, status, updated_at DESC);

CREATE INDEX copytrade_orders_wallet_idx
    ON copytrade_orders (wallet_address, updated_at DESC);

CREATE TABLE copytrade_positions (
    account_id TEXT NOT NULL,
    wallet_address TEXT NOT NULL DEFAULT '',
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL DEFAULT '',
    size NUMERIC(24, 8) NOT NULL DEFAULT 0,
    avg_price NUMERIC(12, 6) NOT NULL DEFAULT 0,
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (account_id, token_id)
);

CREATE INDEX copytrade_positions_account_condition_idx
    ON copytrade_positions (account_id, condition_id);

CREATE TABLE copytrade_account_state (
    account_id TEXT PRIMARY KEY,
    capital_usd NUMERIC(18, 4) NOT NULL CHECK (capital_usd >= 0),
    available_usd NUMERIC(18, 4) NOT NULL,
    reserved_usd NUMERIC(18, 4) NOT NULL DEFAULT 0,
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    fees_paid NUMERIC(14, 4) NOT NULL DEFAULT 0,
    tick_index BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE copytrade_events (
    id TEXT PRIMARY KEY,
    wallet_address TEXT,
    condition_id TEXT,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    message TEXT NOT NULL,
    metadata_json JSONB NOT NULL CHECK (jsonb_typeof(metadata_json) = 'object'),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX copytrade_events_created_at_idx
    ON copytrade_events (created_at DESC);

CREATE INDEX copytrade_events_wallet_created_at_idx
    ON copytrade_events (wallet_address, created_at DESC);
