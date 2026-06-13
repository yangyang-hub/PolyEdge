ALTER TABLE markets
    ADD COLUMN liquidity_usd NUMERIC(18, 2) NOT NULL DEFAULT 0
        CHECK (liquidity_usd >= 0),
    ADD COLUMN end_at TIMESTAMPTZ,
    ADD COLUMN synced_at TIMESTAMPTZ NOT NULL DEFAULT now();

CREATE INDEX idx_markets_reward_quality
    ON markets (
        status,
        tradability_status,
        ambiguity_level,
        liquidity_usd DESC,
        volume_24h DESC,
        end_at DESC,
        synced_at DESC
    )
    WHERE polymarket_condition_id IS NOT NULL;

-- Runtime versions before this migration could locally mark an unresolved
-- external order as cancelled after a timeout. Restore those rows to a locked
-- reconciliation state so a potentially live or filled exchange order cannot
-- be forgotten and replaced with duplicate exposure.
UPDATE reward_managed_orders
SET status = CASE
        WHEN external_order_id IS NULL THEN 'planned'
        ELSE 'open'
    END,
    scoring = false,
    reason = CASE
        WHEN external_order_id IS NULL THEN
            'live submission attempted; live submission result unknown; manual reconciliation required; restored by migration 0037'
        ELSE
            'awaiting final reconciliation; stale local auto-cancel restored by migration 0037: '
                || external_order_id
    END,
    updated_at = now()
WHERE status = 'cancelled'
  AND reason LIKE 'auto-cancelled stale order:%';
