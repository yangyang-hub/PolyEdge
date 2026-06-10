-- 0033_auto_cancel_not_found_orders.sql
-- Auto-cancel reward managed orders that were stuck in "open" status because
-- Polymarket returned 404 (ORDER_NOT_FOUND) but the old code kept them open
-- with a "manual reconciliation required" marker instead of cancelling them.

UPDATE reward_managed_orders
SET
    status = 'cancelled',
    scoring = false,
    reason = reason || ' [auto-cancelled by migration 0033]',
    updated_at = now()
WHERE status = 'open'
  AND reason LIKE '%LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER%';
