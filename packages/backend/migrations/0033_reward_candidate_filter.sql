-- Supports the reward candidate market query with pushed-down filters.
-- The partial index covers active reward markets with valid tokens and spread,
-- enabling PostgreSQL to skip rows that fail these checks during the candidate scan.
CREATE INDEX IF NOT EXISTS idx_reward_candidates_filtered
    ON reward_markets (total_daily_rate DESC, updated_at DESC)
    WHERE active = true
      AND rewards_max_spread > 0
      AND jsonb_array_length(tokens_json) >= 2;
