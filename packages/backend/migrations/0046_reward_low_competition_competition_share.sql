ALTER TABLE reward_low_competition_observations
    ADD COLUMN competition_probe_notional_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    ADD COLUMN competition_share_bps NUMERIC(20,8) NOT NULL DEFAULT 0,
    ADD COLUMN competition_multiple NUMERIC(20,8) NOT NULL DEFAULT 0,
    ADD COLUMN account_effective_available_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    ADD COLUMN low_competition_open_buy_notional_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    ADD COLUMN low_competition_open_buy_notional_usd_after_plan NUMERIC(20,8) NOT NULL DEFAULT 0,
    ADD COLUMN condition_buy_notional_usd_after_plan NUMERIC(20,8) NOT NULL DEFAULT 0,
    ADD COLUMN account_allocation_bps NUMERIC(20,8) NOT NULL DEFAULT 0,
    ADD COLUMN market_allocation_bps NUMERIC(20,8) NOT NULL DEFAULT 0;
