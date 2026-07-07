ALTER TABLE reward_managed_orders
    ADD COLUMN strategy_bucket TEXT NOT NULL DEFAULT 'standard'
        CHECK (strategy_bucket IN ('standard', 'none'));

CREATE INDEX reward_managed_orders_bucket_status_idx
    ON reward_managed_orders (strategy_bucket, account_id, status, updated_at DESC);
