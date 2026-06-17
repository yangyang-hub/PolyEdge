-- Keep high-frequency market freshness refreshes out of the rewards quality
-- index. Rewards queries still filter synced_at from the heap row after using
-- the stable market quality columns and reward_markets join keys.
DROP INDEX IF EXISTS idx_markets_reward_quality;

CREATE INDEX idx_markets_reward_quality
    ON markets (
        status,
        tradability_status,
        ambiguity_level,
        liquidity_usd DESC,
        volume_24h DESC,
        end_at DESC
    )
    WHERE polymarket_condition_id IS NOT NULL;
