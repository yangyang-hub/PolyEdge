CREATE TABLE orders (
    id TEXT PRIMARY KEY,
    signal_id TEXT NOT NULL REFERENCES signals (id) ON DELETE RESTRICT,
    execution_request_id TEXT NOT NULL REFERENCES execution_requests (id) ON DELETE RESTRICT,
    order_draft_id TEXT NOT NULL REFERENCES order_drafts (id) ON DELETE RESTRICT,
    market_id TEXT NOT NULL REFERENCES markets (id) ON DELETE RESTRICT,
    connector_name TEXT NOT NULL,
    account_id TEXT NOT NULL,
    external_order_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    limit_price NUMERIC(12, 6) NOT NULL CHECK (limit_price >= 0 AND limit_price <= 1),
    quantity NUMERIC(20, 8) NOT NULL CHECK (quantity >= 0),
    filled_quantity NUMERIC(20, 8) NOT NULL CHECK (filled_quantity >= 0),
    avg_fill_price NUMERIC(12, 6) NOT NULL CHECK (avg_fill_price >= 0 AND avg_fill_price <= 1),
    status TEXT NOT NULL CHECK (status IN (
        'new',
        'submitted',
        'open',
        'partially_filled',
        'filled',
        'canceled',
        'expired',
        'rejected'
    )),
    submitted_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL,
    version BIGINT NOT NULL
);

CREATE UNIQUE INDEX orders_execution_request_id_idx
    ON orders (execution_request_id);

CREATE UNIQUE INDEX orders_order_draft_id_idx
    ON orders (order_draft_id);

CREATE UNIQUE INDEX orders_connector_external_order_id_idx
    ON orders (connector_name, external_order_id);

CREATE INDEX orders_signal_status_updated_at_idx
    ON orders (signal_id, status, updated_at DESC);

CREATE INDEX orders_market_updated_at_idx
    ON orders (market_id, updated_at DESC);

CREATE TABLE trades (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL REFERENCES orders (id) ON DELETE RESTRICT,
    signal_id TEXT NOT NULL REFERENCES signals (id) ON DELETE RESTRICT,
    market_id TEXT NOT NULL REFERENCES markets (id) ON DELETE RESTRICT,
    connector_name TEXT NOT NULL,
    external_trade_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    price NUMERIC(12, 6) NOT NULL CHECK (price >= 0 AND price <= 1),
    quantity NUMERIC(20, 8) NOT NULL CHECK (quantity >= 0),
    fee NUMERIC(12, 2) NOT NULL CHECK (fee >= 0),
    executed_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX trades_connector_external_trade_id_idx
    ON trades (connector_name, external_trade_id);

CREATE INDEX trades_order_executed_at_idx
    ON trades (order_id, executed_at DESC);

CREATE INDEX trades_market_executed_at_idx
    ON trades (market_id, executed_at DESC);

CREATE TABLE positions (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL REFERENCES markets (id) ON DELETE RESTRICT,
    connector_name TEXT NOT NULL,
    account_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    net_quantity NUMERIC(20, 8) NOT NULL CHECK (net_quantity >= 0),
    avg_cost NUMERIC(12, 6) NOT NULL CHECK (avg_cost >= 0 AND avg_cost <= 1),
    mark_price NUMERIC(12, 6) NOT NULL CHECK (mark_price >= 0 AND mark_price <= 1),
    unrealized_pnl NUMERIC(14, 2) NOT NULL,
    realized_pnl NUMERIC(14, 2) NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL,
    version BIGINT NOT NULL
);

CREATE UNIQUE INDEX positions_connector_account_market_side_idx
    ON positions (connector_name, account_id, market_id, side);

CREATE INDEX positions_market_updated_at_idx
    ON positions (market_id, updated_at DESC);
