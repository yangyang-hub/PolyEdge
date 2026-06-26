-- 0048_reward_account_unmanaged_buy_notional.sql
-- Snapshot-frozen notional of active buy orders on Polymarket that are NOT
-- tracked as bot-managed (i.e. true external/unknown buy occupancy). Computed
-- once per CLOB open-order snapshot from `external_buy_notional - managed` using
-- values from the same snapshot, so it stays stable between snapshots and is
-- not perturbed by the bot cancelling its own managed buys between snapshots.
-- Funding precheck reads this directly instead of recomputing
-- `external_buy_notional(old) - managed(now)`, which used to spike whenever
-- managed buys were cancelled and made eligible_markets oscillate to 0.

ALTER TABLE reward_account_state
    ADD COLUMN IF NOT EXISTS unmanaged_external_buy_notional NUMERIC(18, 4) NOT NULL DEFAULT 0;
