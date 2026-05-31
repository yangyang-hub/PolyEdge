CREATE TABLE reward_control_commands (
    id TEXT PRIMARY KEY,
    action TEXT NOT NULL CHECK (action IN ('run_once', 'cancel_all', 'reset')),
    account_id TEXT,
    reason TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    requested_at TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    trace_id TEXT,
    error TEXT
);

CREATE INDEX reward_control_commands_pending_idx
    ON reward_control_commands (status, requested_at)
    WHERE status = 'pending';

CREATE INDEX reward_control_commands_recent_idx
    ON reward_control_commands (requested_at DESC);
