CREATE TABLE reward_worker_heartbeats (
    account_id TEXT PRIMARY KEY,
    observed_at TIMESTAMPTZ NOT NULL
);
