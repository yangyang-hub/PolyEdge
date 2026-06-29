-- External inventory exit intents may reference markets outside the current
-- rewards catalog, just like reward_positions.
ALTER TABLE reward_managed_orders
    DROP CONSTRAINT IF EXISTS reward_managed_orders_condition_id_fkey;
