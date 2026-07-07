-- Rewards market-maker shadow/guarded decision audit.
-- One row per evaluated quote leg. Live orders can later reference these ids
-- for PnL attribution; this migration only adds the audit sink.
CREATE TABLE reward_market_maker_decisions (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    strategy_mode TEXT NOT NULL CHECK (strategy_mode IN (
        'rewards_only',
        'market_maker_shadow',
        'market_maker_guarded'
    )),
    decision_type TEXT NOT NULL CHECK (decision_type IN (
        'quote',
        'skip',
        'cancel',
        'hold',
        'exit',
        'merge'
    )),
    decision_status TEXT NOT NULL CHECK (decision_status IN (
        'allowed',
        'blocked',
        'shadow_allowed',
        'shadow_blocked'
    )),
    target_price NUMERIC CHECK (target_price IS NULL OR (target_price >= 0 AND target_price <= 1)),
    target_size NUMERIC CHECK (target_size IS NULL OR target_size >= 0),
    target_notional_usd NUMERIC CHECK (target_notional_usd IS NULL OR target_notional_usd >= 0),
    fair_value_id BIGINT REFERENCES reward_market_fair_values(id) ON DELETE SET NULL,
    reward_ev_id BIGINT,
    pricing_edge_cents NUMERIC NOT NULL,
    reward_ev_cents NUMERIC NOT NULL,
    exit_cost_cents NUMERIC NOT NULL,
    adverse_selection_cost_cents NUMERIC NOT NULL,
    inventory_penalty_cents NUMERIC NOT NULL,
    uncertainty_buffer_cents NUMERIC NOT NULL,
    total_ev_cents NUMERIC NOT NULL,
    max_profitable_bid NUMERIC CHECK (
        max_profitable_bid IS NULL OR (max_profitable_bid >= 0 AND max_profitable_bid <= 1)
    ),
    reason_codes JSONB NOT NULL DEFAULT '[]',
    inputs_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reward_market_maker_decisions_account_created_idx
    ON reward_market_maker_decisions (account_id, created_at DESC);

CREATE INDEX reward_market_maker_decisions_condition_created_idx
    ON reward_market_maker_decisions (condition_id, created_at DESC);

CREATE INDEX reward_market_maker_decisions_status_created_idx
    ON reward_market_maker_decisions (decision_status, created_at DESC);
