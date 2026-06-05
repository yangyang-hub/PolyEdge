-- Remove paper_trade, manual_confirm, and research mode variants.
-- Only live_auto and kill_switch_locked remain.
-- Existing rows are migrated to 'live_auto' before constraints are tightened.

-- 1. Migrate existing data
UPDATE system_runtime_state SET mode = 'live_auto' WHERE mode NOT IN ('live_auto', 'kill_switch_locked');
UPDATE mode_transitions SET from_mode = 'live_auto' WHERE from_mode NOT IN ('live_auto', 'kill_switch_locked');
UPDATE mode_transitions SET to_mode = 'live_auto' WHERE to_mode NOT IN ('live_auto', 'kill_switch_locked');
UPDATE execution_requests SET mode = 'live_auto' WHERE mode NOT IN ('live_auto', 'kill_switch_locked');

-- 2. Replace CHECK constraints (drop old, add new)
ALTER TABLE system_runtime_state DROP CONSTRAINT system_runtime_state_mode_check;
ALTER TABLE system_runtime_state ADD CONSTRAINT system_runtime_state_mode_check
    CHECK (mode IN ('live_auto', 'kill_switch_locked'));

ALTER TABLE mode_transitions DROP CONSTRAINT mode_transitions_from_mode_check;
ALTER TABLE mode_transitions ADD CONSTRAINT mode_transitions_from_mode_check
    CHECK (from_mode IN ('live_auto', 'kill_switch_locked'));

ALTER TABLE mode_transitions DROP CONSTRAINT mode_transitions_to_mode_check;
ALTER TABLE mode_transitions ADD CONSTRAINT mode_transitions_to_mode_check
    CHECK (to_mode IN ('live_auto', 'kill_switch_locked'));

ALTER TABLE execution_requests DROP CONSTRAINT execution_requests_mode_check;
ALTER TABLE execution_requests ADD CONSTRAINT execution_requests_mode_check
    CHECK (mode IN ('live_auto', 'kill_switch_locked'));
