-- 0034_reward_account_external_buy_notional.sql
-- Track total notional of all active buy orders on Polymarket (bot-managed +
-- external) for CLOB balance pre-check during order placement.

ALTER TABLE reward_account_state
    ADD COLUMN IF NOT EXISTS external_buy_notional NUMERIC(18, 4) NOT NULL DEFAULT 0;
