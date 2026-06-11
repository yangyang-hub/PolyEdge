-- Restore managed orders that runtime code incorrectly auto-cancelled when the
-- single-order endpoint returned 404. A missing order can still have confirmed
-- account trades, so the worker must reconcile those trades before deciding
-- whether the order is terminal.

UPDATE reward_managed_orders
SET
    status = 'open',
    scoring = false,
    reason = 'external order lookup returned not found; manual reconciliation required: '
        || external_order_id,
    updated_at = now()
WHERE status = 'cancelled'
  AND external_order_id IS NOT NULL
  AND reason LIKE 'order not found on Polymarket (404); auto-cancelled:%';
