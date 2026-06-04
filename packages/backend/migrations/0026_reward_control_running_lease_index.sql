CREATE INDEX IF NOT EXISTS idx_reward_control_commands_running_started_at
ON reward_control_commands (started_at, requested_at)
WHERE status = 'running';
