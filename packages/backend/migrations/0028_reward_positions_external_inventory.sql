-- Rewards account reconciliation stores the complete Polymarket wallet inventory,
-- including positions in markets that are not currently in the rewards catalog.
ALTER TABLE reward_positions
    DROP CONSTRAINT IF EXISTS reward_positions_condition_id_fkey;
