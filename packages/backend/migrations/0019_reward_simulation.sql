-- Stateful rewards market-making simulation: order fills, fund-pool ledger.

ALTER TABLE reward_managed_orders
    ADD COLUMN filled_size NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (filled_size >= 0),
    ADD COLUMN reward_earned NUMERIC(14, 4) NOT NULL DEFAULT 0,
    ADD COLUMN last_scored_at TIMESTAMPTZ;

CREATE TABLE reward_fills (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 6) NOT NULL CHECK (price > 0 AND price < 1),
    size NUMERIC(24, 8) NOT NULL CHECK (size > 0),
    notional_usd NUMERIC(18, 4) NOT NULL CHECK (notional_usd >= 0),
    role TEXT NOT NULL CHECK (role IN ('maker', 'taker')),
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    reason TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX reward_fills_account_created_at_idx
    ON reward_fills (account_id, created_at DESC);

CREATE INDEX reward_fills_condition_created_at_idx
    ON reward_fills (condition_id, created_at DESC);

CREATE TABLE reward_account_state (
    account_id TEXT PRIMARY KEY,
    capital_usd NUMERIC(18, 4) NOT NULL CHECK (capital_usd >= 0),
    available_usd NUMERIC(18, 4) NOT NULL,
    reserved_usd NUMERIC(18, 4) NOT NULL DEFAULT 0,
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    reward_earned_usd NUMERIC(14, 4) NOT NULL DEFAULT 0,
    fees_paid NUMERIC(14, 4) NOT NULL DEFAULT 0,
    tick_index BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL
);
