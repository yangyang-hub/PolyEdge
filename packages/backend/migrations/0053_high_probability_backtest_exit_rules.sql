ALTER TABLE high_probability_backtest_runs
    ADD COLUMN exit_rule_reports JSONB NOT NULL DEFAULT '[]';
