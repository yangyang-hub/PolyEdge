WITH ranked_active_reward_commands AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY action, account_id
            ORDER BY
                CASE status WHEN 'running' THEN 0 ELSE 1 END,
                requested_at,
                id
        ) AS duplicate_rank
    FROM reward_control_commands
    WHERE status IN ('pending', 'running')
)
UPDATE reward_control_commands AS cmd
SET status = 'completed',
    completed_at = COALESCE(cmd.completed_at, now()),
    error = COALESCE(
        cmd.error,
        'coalesced duplicate pending/running command during migration 0045'
    )
FROM ranked_active_reward_commands AS ranked
WHERE cmd.id = ranked.id
  AND ranked.duplicate_rank > 1;

CREATE UNIQUE INDEX IF NOT EXISTS reward_control_commands_active_account_dedupe_idx
    ON reward_control_commands (action, account_id)
    WHERE status IN ('pending', 'running')
      AND account_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS reward_control_commands_active_global_dedupe_idx
    ON reward_control_commands (action)
    WHERE status IN ('pending', 'running')
      AND account_id IS NULL;
