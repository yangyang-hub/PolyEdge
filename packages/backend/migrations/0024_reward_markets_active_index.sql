-- Index for reward_markets snapshot query: active=true ORDER BY total_daily_rate DESC, updated_at DESC
CREATE INDEX IF NOT EXISTS idx_reward_markets_active_daily_rate
    ON reward_markets (active, total_daily_rate DESC, updated_at DESC)
    WHERE active = true;
