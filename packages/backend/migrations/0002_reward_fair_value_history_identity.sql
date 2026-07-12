-- Preserve the immutable 0001 checksum for existing deployments while adding
-- idempotent fair-value history writes. Keep one arbitrary physical row for
-- each historical identity before creating the unique index.
DELETE FROM reward_fair_value_history AS duplicate
USING reward_fair_value_history AS keeper
WHERE duplicate.condition_id = keeper.condition_id
  AND duplicate.source = keeper.source
  AND duplicate.observed_at = keeper.observed_at
  AND duplicate.ctid > keeper.ctid;

CREATE UNIQUE INDEX IF NOT EXISTS reward_fair_value_history_identity_uidx
    ON reward_fair_value_history (condition_id, source, observed_at);
