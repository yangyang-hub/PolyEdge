-- Add indexes for rewards snapshot queries that previously required full table scans.
--
-- pg_trgm GIN indexes:  reward_managed_orders search uses ILIKE '%val%' on
--   outcome, condition_id, and token_id.  Leading-wildcard ILIKE cannot use
--   B-tree indexes.  pg_trgm GIN indexes support these patterns efficiently.
--
-- reward_quote_plans JSON text search uses quote_plan_json::text ILIKE '%val%'.
--   An expression GIN index on the cast allows pg_trgm to serve the query.
--
-- reward_fills_created_at_idx:  fallback for unfiltered ORDER BY created_at DESC.
--   The primary query path now filters by account_id and uses the existing
--   (account_id, created_at DESC) composite index, but this standalone index
--   covers any code path that queries fills without an account_id filter.
--
-- reward_positions_updated_at_idx:  partial index for WHERE size <> 0
--   ORDER BY updated_at DESC.  The primary query path now filters by account_id
--   (using the PK prefix), but this covers any code path querying positions
--   across accounts.

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS reward_managed_orders_outcome_trgm_idx
    ON reward_managed_orders USING GIN (outcome gin_trgm_ops);

CREATE INDEX IF NOT EXISTS reward_managed_orders_condition_id_trgm_idx
    ON reward_managed_orders USING GIN (condition_id gin_trgm_ops);

CREATE INDEX IF NOT EXISTS reward_managed_orders_token_id_trgm_idx
    ON reward_managed_orders USING GIN (token_id gin_trgm_ops);

CREATE INDEX IF NOT EXISTS reward_quote_plans_json_trgm_idx
    ON reward_quote_plans USING GIN ((quote_plan_json::text) gin_trgm_ops);

CREATE INDEX IF NOT EXISTS reward_fills_created_at_idx
    ON reward_fills (created_at DESC);

CREATE INDEX IF NOT EXISTS reward_positions_updated_at_idx
    ON reward_positions (updated_at DESC)
    WHERE size <> 0;
