CREATE TABLE high_probability_backtest_runs (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    run_at TIMESTAMPTZ NOT NULL,
    model_version TEXT NOT NULL,
    market_scope TEXT NOT NULL,
    sample_limit BIGINT NOT NULL CHECK (sample_limit > 0),
    train_sample_count BIGINT NOT NULL CHECK (train_sample_count >= 0),
    test_sample_count BIGINT NOT NULL CHECK (test_sample_count >= 0),
    candidate_count BIGINT NOT NULL CHECK (candidate_count >= 0),
    trade_count BIGINT NOT NULL CHECK (trade_count >= 0),
    skipped_no_bucket_count BIGINT NOT NULL CHECK (skipped_no_bucket_count >= 0),
    skipped_no_edge_count BIGINT NOT NULL CHECK (skipped_no_edge_count >= 0),
    win_trades BIGINT NOT NULL CHECK (win_trades >= 0),
    loss_trades BIGINT NOT NULL CHECK (loss_trades >= 0),
    win_rate NUMERIC CHECK (win_rate >= 0 AND win_rate <= 1),
    total_pnl NUMERIC NOT NULL,
    average_pnl NUMERIC,
    total_entry_cost NUMERIC NOT NULL CHECK (total_entry_cost >= 0),
    roi NUMERIC,
    max_drawdown NUMERIC NOT NULL CHECK (max_drawdown >= 0),
    average_entry_price NUMERIC CHECK (average_entry_price >= 0 AND average_entry_price <= 1),
    train_start_at TIMESTAMPTZ,
    train_end_at TIMESTAMPTZ,
    test_start_at TIMESTAMPTZ,
    test_end_at TIMESTAMPTZ,
    notes JSONB NOT NULL DEFAULT '[]',
    config_json JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX high_probability_backtest_runs_recent_idx
    ON high_probability_backtest_runs (run_at DESC);
CREATE INDEX high_probability_backtest_runs_model_idx
    ON high_probability_backtest_runs (model_version, run_at DESC);

CREATE TABLE high_probability_backtest_trades (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    run_id BIGINT NOT NULL REFERENCES high_probability_backtest_runs(id) ON DELETE CASCADE,
    sample_id BIGINT NOT NULL REFERENCES high_probability_samples(id),
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    sampled_at TIMESTAMPTZ NOT NULL,
    bucket_key TEXT NOT NULL,
    executable_price NUMERIC NOT NULL CHECK (executable_price >= 0 AND executable_price <= 1),
    fair_probability NUMERIC NOT NULL CHECK (fair_probability >= 0 AND fair_probability <= 1),
    net_edge NUMERIC NOT NULL,
    recommended_max_entry_price NUMERIC
        CHECK (recommended_max_entry_price >= 0 AND recommended_max_entry_price <= 1),
    outcome TEXT NOT NULL CHECK (outcome IN ('win', 'loss')),
    settlement_pnl NUMERIC NOT NULL,
    cumulative_pnl NUMERIC NOT NULL,
    drawdown NUMERIC NOT NULL CHECK (drawdown >= 0),
    reasons JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX high_probability_backtest_trades_run_idx
    ON high_probability_backtest_trades (run_id, sampled_at, id);
CREATE INDEX high_probability_backtest_trades_condition_idx
    ON high_probability_backtest_trades (condition_id, sampled_at DESC);
