CREATE TABLE order_drafts (
    id TEXT PRIMARY KEY,
    signal_id TEXT NOT NULL REFERENCES signals(id) ON DELETE CASCADE,
    signal_version BIGINT NOT NULL CHECK (signal_version >= 1),
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    connector_name TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    limit_price NUMERIC(12, 6) NOT NULL CHECK (limit_price >= 0 AND limit_price <= 1),
    quantity NUMERIC(20, 8) NOT NULL CHECK (quantity > 0),
    notional NUMERIC(18, 2) NOT NULL CHECK (notional >= 0),
    status TEXT NOT NULL CHECK (status IN ('queued', 'submitted', 'rejected', 'canceled')),
    created_by_user_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL,
    UNIQUE (signal_id, signal_version)
);

CREATE INDEX order_drafts_signal_created_at_idx
    ON order_drafts (signal_id, created_at DESC);

CREATE INDEX order_drafts_status_created_at_idx
    ON order_drafts (status, created_at DESC);

CREATE INDEX order_drafts_connector_created_at_idx
    ON order_drafts (connector_name, created_at DESC);

CREATE TABLE execution_requests (
    id TEXT PRIMARY KEY,
    signal_id TEXT NOT NULL REFERENCES signals(id) ON DELETE CASCADE,
    signal_version BIGINT NOT NULL CHECK (signal_version >= 1),
    order_draft_id TEXT NOT NULL UNIQUE REFERENCES order_drafts(id) ON DELETE CASCADE,
    connector_name TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (
        mode IN ('research', 'paper_trade', 'manual_confirm', 'live_auto', 'kill_switch_locked')
    ),
    risk_state_version BIGINT NOT NULL CHECK (risk_state_version >= 1),
    requested_by_user_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'submitted', 'failed', 'canceled')),
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL,
    UNIQUE (signal_id, signal_version)
);

CREATE INDEX execution_requests_signal_created_at_idx
    ON execution_requests (signal_id, created_at DESC);

CREATE INDEX execution_requests_status_created_at_idx
    ON execution_requests (status, created_at DESC);

CREATE INDEX execution_requests_connector_created_at_idx
    ON execution_requests (connector_name, created_at DESC);
