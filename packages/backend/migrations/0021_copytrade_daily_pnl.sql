-- Add daily_realized_pnl column to copytrade_account_state for per-day PnL tracking.
-- This field resets to zero when the UTC date rolls over (handled by the engine).
-- Used by the daily_loss_limit_usd risk check.

ALTER TABLE copytrade_account_state
    ADD COLUMN daily_realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0;
