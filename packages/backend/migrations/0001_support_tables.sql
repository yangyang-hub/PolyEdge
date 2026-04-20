CREATE TABLE IF NOT EXISTS audit_logs (
  id TEXT PRIMARY KEY,
  occurred_at TIMESTAMPTZ NOT NULL,
  request_id TEXT NOT NULL,
  trace_id TEXT NOT NULL,
  actor_user_id TEXT NOT NULL,
  actor_session_id TEXT NOT NULL,
  actor_roles_json JSONB NOT NULL,
  action TEXT NOT NULL,
  resource_type TEXT NOT NULL,
  resource_id TEXT NOT NULL,
  reason TEXT,
  result TEXT NOT NULL CHECK (result IN ('accepted', 'succeeded', 'rejected', 'failed')),
  error_code TEXT,
  ip TEXT,
  user_agent_summary TEXT,
  payload_json JSONB,
  version_snapshot_json JSONB
);

CREATE INDEX IF NOT EXISTS audit_logs_request_id_idx ON audit_logs (request_id);
CREATE INDEX IF NOT EXISTS audit_logs_trace_id_idx ON audit_logs (trace_id);
CREATE INDEX IF NOT EXISTS audit_logs_resource_idx ON audit_logs (resource_type, resource_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS audit_logs_actor_idx ON audit_logs (actor_user_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS audit_logs_action_idx ON audit_logs (action, occurred_at DESC);

CREATE TABLE IF NOT EXISTS idempotency_keys (
  scope TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  request_id TEXT NOT NULL,
  actor_user_id TEXT,
  actor_session_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('started', 'completed', 'failed')),
  resource_type TEXT,
  resource_id TEXT,
  response_json JSONB,
  first_seen_at TIMESTAMPTZ NOT NULL,
  last_seen_at TIMESTAMPTZ NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (scope, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idempotency_keys_expires_at_idx ON idempotency_keys (expires_at);
CREATE INDEX IF NOT EXISTS idempotency_keys_request_id_idx ON idempotency_keys (request_id);

CREATE TABLE IF NOT EXISTS outbox_events (
  id TEXT PRIMARY KEY,
  event_id TEXT NOT NULL UNIQUE,
  aggregate_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload_json JSONB NOT NULL,
  trace_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'published', 'failed', 'dead_letter')),
  delivery_attempts INTEGER NOT NULL DEFAULT 0,
  next_attempt_at TIMESTAMPTZ,
  published_at TIMESTAMPTZ,
  last_error TEXT,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS outbox_events_pending_idx ON outbox_events (status, next_attempt_at, created_at);
CREATE INDEX IF NOT EXISTS outbox_events_aggregate_idx ON outbox_events (aggregate_type, aggregate_id, created_at DESC);

CREATE TABLE IF NOT EXISTS external_event_dedup (
  source_system TEXT NOT NULL,
  external_event_id TEXT NOT NULL,
  payload_hash TEXT NOT NULL,
  first_seen_at TIMESTAMPTZ NOT NULL,
  processed_at TIMESTAMPTZ,
  trace_id TEXT NOT NULL,
  PRIMARY KEY (source_system, external_event_id)
);

CREATE INDEX IF NOT EXISTS external_event_dedup_processed_idx ON external_event_dedup (processed_at);

CREATE TABLE IF NOT EXISTS llm_calls (
  id TEXT PRIMARY KEY,
  task_type TEXT NOT NULL,
  model_version TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  input_hash TEXT NOT NULL,
  raw_output JSONB,
  parsed_output JSONB,
  validation_result JSONB NOT NULL,
  fallback_used BOOLEAN NOT NULL DEFAULT FALSE,
  latency_ms BIGINT NOT NULL,
  cost_estimate NUMERIC(24,8),
  trace_id TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS llm_calls_task_type_idx ON llm_calls (task_type, created_at DESC);
CREATE INDEX IF NOT EXISTS llm_calls_prompt_version_idx ON llm_calls (prompt_version, created_at DESC);
CREATE INDEX IF NOT EXISTS llm_calls_input_hash_idx ON llm_calls (input_hash);
CREATE INDEX IF NOT EXISTS llm_calls_trace_id_idx ON llm_calls (trace_id);

CREATE TABLE IF NOT EXISTS system_runtime_state (
  id TEXT PRIMARY KEY,
  mode TEXT NOT NULL CHECK (
    mode IN ('research', 'paper_trade', 'manual_confirm', 'live_auto', 'kill_switch_locked')
  ),
  environment TEXT NOT NULL,
  version BIGINT NOT NULL DEFAULT 1,
  updated_at TIMESTAMPTZ NOT NULL,
  trace_id TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS system_runtime_state_updated_at_idx ON system_runtime_state (updated_at DESC);

CREATE TABLE IF NOT EXISTS mode_transitions (
  id TEXT PRIMARY KEY,
  from_mode TEXT NOT NULL CHECK (
    from_mode IN ('research', 'paper_trade', 'manual_confirm', 'live_auto', 'kill_switch_locked')
  ),
  to_mode TEXT NOT NULL CHECK (
    to_mode IN ('research', 'paper_trade', 'manual_confirm', 'live_auto', 'kill_switch_locked')
  ),
  reason TEXT NOT NULL,
  requested_by_user_id TEXT NOT NULL,
  requested_by_session_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  trace_id TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS mode_transitions_request_id_idx ON mode_transitions (request_id);
CREATE INDEX IF NOT EXISTS mode_transitions_created_at_idx ON mode_transitions (created_at DESC);
