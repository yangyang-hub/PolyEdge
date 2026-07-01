-- PolyEdge complete PostgreSQL initialization script
-- Generated from packages/backend/migrations/*.sql on 2026-06-19.
-- Intended for initializing an empty database. Runtime sqlx migrations remain in packages/backend/migrations.


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0001_support_tables.sql
-------------------------------------------------------------------------------

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


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0002_market_event_core.sql
-------------------------------------------------------------------------------

CREATE TABLE markets (
    id TEXT PRIMARY KEY,
    question TEXT NOT NULL,
    category TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('open', 'closed', 'resolved')),
    best_bid NUMERIC(12, 6) NOT NULL CHECK (best_bid >= 0 AND best_bid <= 1),
    best_ask NUMERIC(12, 6) NOT NULL CHECK (best_ask >= 0 AND best_ask <= 1),
    mid_price NUMERIC(12, 6) NOT NULL CHECK (mid_price >= 0 AND mid_price <= 1),
    volume_24h NUMERIC(18, 2) NOT NULL CHECK (volume_24h >= 0),
    ambiguity_level TEXT NOT NULL CHECK (ambiguity_level IN ('low', 'medium', 'high')),
    tradability_status TEXT NOT NULL CHECK (
        tradability_status IN ('tradable', 'manual_review', 'observe_only', 'blocked')
    ),
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX markets_status_updated_at_idx
    ON markets (status, updated_at DESC);

CREATE INDEX markets_tradability_status_updated_at_idx
    ON markets (tradability_status, updated_at DESC);

CREATE TABLE market_resolution_rules (
    market_id TEXT PRIMARY KEY REFERENCES markets(id) ON DELETE CASCADE,
    resolution_source TEXT NOT NULL,
    edge_case_notes TEXT[] NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX market_resolution_rules_updated_at_idx
    ON market_resolution_rules (updated_at DESC);

CREATE TABLE raw_events (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    hash TEXT NOT NULL,
    raw_payload JSONB NOT NULL CHECK (jsonb_typeof(raw_payload) = 'object'),
    ingested_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL
);

CREATE UNIQUE INDEX raw_events_source_hash_uidx
    ON raw_events (source, hash);

CREATE INDEX raw_events_ingested_at_idx
    ON raw_events (ingested_at DESC);

CREATE TABLE events (
    id TEXT PRIMARY KEY,
    raw_event_id TEXT REFERENCES raw_events(id) ON DELETE SET NULL,
    source TEXT NOT NULL,
    summary TEXT NOT NULL,
    relevance_score NUMERIC(12, 6) NOT NULL CHECK (relevance_score >= 0 AND relevance_score <= 1),
    confidence NUMERIC(12, 6) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    status TEXT NOT NULL CHECK (status IN ('active', 'expired', 'invalidated', 'superseded')),
    reason_trace TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX events_status_created_at_idx
    ON events (status, created_at DESC);

CREATE INDEX events_updated_at_idx
    ON events (updated_at DESC);

CREATE INDEX events_raw_event_id_idx
    ON events (raw_event_id);

CREATE TABLE event_market_links (
    event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (event_id, market_id)
);

CREATE INDEX event_market_links_market_id_idx
    ON event_market_links (market_id);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0003_evidence_signal_core.sql
-------------------------------------------------------------------------------

CREATE TABLE evidences (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    direction TEXT NOT NULL CHECK (direction IN ('supports_yes', 'supports_no', 'background')),
    strength NUMERIC(12, 6) NOT NULL CHECK (strength >= 0 AND strength <= 1),
    source_reliability NUMERIC(12, 6) NOT NULL CHECK (source_reliability >= 0 AND source_reliability <= 1),
    novelty NUMERIC(12, 6) NOT NULL CHECK (novelty >= 0 AND novelty <= 1),
    resolution_relevance NUMERIC(12, 6) NOT NULL CHECK (resolution_relevance >= 0 AND resolution_relevance <= 1),
    status TEXT NOT NULL CHECK (status IN ('active', 'expired', 'invalidated')),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX evidences_market_status_created_at_idx
    ON evidences (market_id, status, created_at DESC);

CREATE INDEX evidences_event_id_idx
    ON evidences (event_id);

CREATE INDEX evidences_status_created_at_idx
    ON evidences (status, created_at DESC);

CREATE TABLE signals (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    action TEXT NOT NULL CHECK (action IN ('buy', 'sell')),
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    market_price NUMERIC(12, 6) NOT NULL CHECK (market_price >= 0 AND market_price <= 1),
    fair_price NUMERIC(12, 6) NOT NULL CHECK (fair_price >= 0 AND fair_price <= 1),
    edge NUMERIC(12, 6) NOT NULL CHECK (edge >= -1 AND edge <= 1),
    confidence NUMERIC(12, 6) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    lifecycle_state TEXT NOT NULL CHECK (
        lifecycle_state IN ('new', 'active', 'weakened', 'executed', 'invalidated', 'reversed', 'expired')
    ),
    reason TEXT NOT NULL,
    risk_decision TEXT NOT NULL,
    approved_by_user_id TEXT,
    approved_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX signals_market_lifecycle_updated_at_idx
    ON signals (market_id, lifecycle_state, updated_at DESC);

CREATE INDEX signals_event_id_idx
    ON signals (event_id);

CREATE INDEX signals_lifecycle_updated_at_idx
    ON signals (lifecycle_state, updated_at DESC);

CREATE TABLE signal_evidence_links (
    signal_id TEXT NOT NULL REFERENCES signals(id) ON DELETE CASCADE,
    evidence_id TEXT NOT NULL REFERENCES evidences(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (signal_id, evidence_id)
);

CREATE INDEX signal_evidence_links_evidence_id_idx
    ON signal_evidence_links (evidence_id);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0004_pricing_and_signal_transitions.sql
-------------------------------------------------------------------------------

CREATE TABLE probability_estimates (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    signal_id TEXT REFERENCES signals(id) ON DELETE SET NULL,
    prior_price NUMERIC(12, 6) NOT NULL CHECK (prior_price >= 0 AND prior_price <= 1),
    posterior_price NUMERIC(12, 6) NOT NULL CHECK (posterior_price >= 0 AND posterior_price <= 1),
    fair_price NUMERIC(12, 6) NOT NULL CHECK (fair_price >= 0 AND fair_price <= 1),
    market_price NUMERIC(12, 6) NOT NULL CHECK (market_price >= 0 AND market_price <= 1),
    edge NUMERIC(12, 6) NOT NULL CHECK (edge >= -1 AND edge <= 1),
    confidence NUMERIC(12, 6) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    time_horizon TEXT NOT NULL CHECK (time_horizon IN ('short', 'medium', 'long')),
    model_version TEXT NOT NULL,
    reason_codes_json JSONB NOT NULL CHECK (jsonb_typeof(reason_codes_json) = 'array'),
    evidence_count INTEGER NOT NULL CHECK (evidence_count >= 0),
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX probability_estimates_market_created_at_idx
    ON probability_estimates (market_id, created_at DESC);

CREATE INDEX probability_estimates_signal_created_at_idx
    ON probability_estimates (signal_id, created_at DESC);

ALTER TABLE signals
    ADD COLUMN estimate_id TEXT REFERENCES probability_estimates(id) ON DELETE SET NULL;

CREATE INDEX signals_estimate_id_idx
    ON signals (estimate_id);

CREATE TABLE signal_transitions (
    id TEXT PRIMARY KEY,
    signal_id TEXT NOT NULL REFERENCES signals(id) ON DELETE CASCADE,
    from_state TEXT NOT NULL CHECK (
        from_state IN ('new', 'active', 'weakened', 'executed', 'invalidated', 'reversed', 'expired')
    ),
    to_state TEXT NOT NULL CHECK (
        to_state IN ('new', 'active', 'weakened', 'executed', 'invalidated', 'reversed', 'expired')
    ),
    trigger_type TEXT NOT NULL,
    trigger_payload_json JSONB NOT NULL CHECK (jsonb_typeof(trigger_payload_json) = 'object'),
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX signal_transitions_signal_created_at_idx
    ON signal_transitions (signal_id, created_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0005_risk_state.sql
-------------------------------------------------------------------------------

CREATE TABLE risk_state (
    id TEXT PRIMARY KEY,
    kill_switch BOOLEAN NOT NULL DEFAULT FALSE,
    daily_pnl NUMERIC(18, 2) NOT NULL,
    gross_exposure NUMERIC(12, 6) NOT NULL CHECK (gross_exposure >= 0 AND gross_exposure <= 10),
    net_exposure NUMERIC(12, 6) NOT NULL CHECK (net_exposure >= 0 AND net_exposure <= 10),
    open_alerts INTEGER NOT NULL DEFAULT 0 CHECK (open_alerts >= 0),
    notes TEXT[] NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX risk_state_updated_at_idx
    ON risk_state (updated_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0006_signal_rejection_metadata.sql
-------------------------------------------------------------------------------

ALTER TABLE signals
    ADD COLUMN rejected_by_user_id TEXT,
    ADD COLUMN rejected_at TIMESTAMPTZ;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0007_execution_request_core.sql
-------------------------------------------------------------------------------

CREATE TABLE order_drafts (
    id TEXT PRIMARY KEY,
    signal_id TEXT NOT NULL REFERENCES signals(id) ON DELETE CASCADE,
    signal_version BIGINT NOT NULL CHECK (signal_version >= 1),
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    connector_name TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    limit_price NUMERIC(12, 6) NOT NULL CHECK (limit_price >= 0 AND limit_price <= 1),
    quantity NUMERIC(20, 8) NOT NULL CHECK (quantity > 0),
    notional NUMERIC(18, 2) NOT NULL CHECK (notional >= 0),
    status TEXT NOT NULL CHECK (status IN ('queued', 'submitted', 'rejected', 'canceled')),
    created_by_user_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL,
    UNIQUE (signal_id, signal_version)
);

CREATE INDEX order_drafts_signal_created_at_idx
    ON order_drafts (signal_id, created_at DESC);

CREATE INDEX order_drafts_status_created_at_idx
    ON order_drafts (status, created_at DESC);

CREATE INDEX order_drafts_connector_created_at_idx
    ON order_drafts (connector_name, created_at DESC);

CREATE TABLE execution_requests (
    id TEXT PRIMARY KEY,
    signal_id TEXT NOT NULL REFERENCES signals(id) ON DELETE CASCADE,
    signal_version BIGINT NOT NULL CHECK (signal_version >= 1),
    order_draft_id TEXT NOT NULL UNIQUE REFERENCES order_drafts(id) ON DELETE CASCADE,
    connector_name TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (
        mode IN ('research', 'paper_trade', 'manual_confirm', 'live_auto', 'kill_switch_locked')
    ),
    risk_state_version BIGINT NOT NULL CHECK (risk_state_version >= 1),
    requested_by_user_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'submitted', 'failed', 'canceled')),
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL,
    UNIQUE (signal_id, signal_version)
);

CREATE INDEX execution_requests_signal_created_at_idx
    ON execution_requests (signal_id, created_at DESC);

CREATE INDEX execution_requests_status_created_at_idx
    ON execution_requests (status, created_at DESC);

CREATE INDEX execution_requests_connector_created_at_idx
    ON execution_requests (connector_name, created_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0008_execution_dispatch_metadata.sql
-------------------------------------------------------------------------------

ALTER TABLE order_drafts
    ADD COLUMN external_order_id TEXT,
    ADD COLUMN submitted_at TIMESTAMPTZ,
    ADD COLUMN failure_code TEXT,
    ADD COLUMN failure_message TEXT;

CREATE INDEX order_drafts_external_order_id_idx
    ON order_drafts (external_order_id)
    WHERE external_order_id IS NOT NULL;

ALTER TABLE execution_requests
    ADD COLUMN external_order_id TEXT,
    ADD COLUMN submitted_at TIMESTAMPTZ,
    ADD COLUMN failure_code TEXT,
    ADD COLUMN failure_message TEXT;

CREATE INDEX execution_requests_external_order_id_idx
    ON execution_requests (external_order_id)
    WHERE external_order_id IS NOT NULL;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0009_orders_trades_positions.sql
-------------------------------------------------------------------------------

CREATE TABLE orders (
    id TEXT PRIMARY KEY,
    signal_id TEXT NOT NULL REFERENCES signals (id) ON DELETE RESTRICT,
    execution_request_id TEXT NOT NULL REFERENCES execution_requests (id) ON DELETE RESTRICT,
    order_draft_id TEXT NOT NULL REFERENCES order_drafts (id) ON DELETE RESTRICT,
    market_id TEXT NOT NULL REFERENCES markets (id) ON DELETE RESTRICT,
    connector_name TEXT NOT NULL,
    account_id TEXT NOT NULL,
    external_order_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    limit_price NUMERIC(12, 6) NOT NULL CHECK (limit_price >= 0 AND limit_price <= 1),
    quantity NUMERIC(20, 8) NOT NULL CHECK (quantity >= 0),
    filled_quantity NUMERIC(20, 8) NOT NULL CHECK (filled_quantity >= 0),
    avg_fill_price NUMERIC(12, 6) NOT NULL CHECK (avg_fill_price >= 0 AND avg_fill_price <= 1),
    status TEXT NOT NULL CHECK (status IN (
        'new',
        'submitted',
        'open',
        'partially_filled',
        'filled',
        'canceled',
        'expired',
        'rejected'
    )),
    submitted_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL,
    version BIGINT NOT NULL
);

CREATE UNIQUE INDEX orders_execution_request_id_idx
    ON orders (execution_request_id);

CREATE UNIQUE INDEX orders_order_draft_id_idx
    ON orders (order_draft_id);

CREATE UNIQUE INDEX orders_connector_external_order_id_idx
    ON orders (connector_name, external_order_id);

CREATE INDEX orders_signal_status_updated_at_idx
    ON orders (signal_id, status, updated_at DESC);

CREATE INDEX orders_market_updated_at_idx
    ON orders (market_id, updated_at DESC);

CREATE TABLE trades (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL REFERENCES orders (id) ON DELETE RESTRICT,
    signal_id TEXT NOT NULL REFERENCES signals (id) ON DELETE RESTRICT,
    market_id TEXT NOT NULL REFERENCES markets (id) ON DELETE RESTRICT,
    connector_name TEXT NOT NULL,
    external_trade_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    price NUMERIC(12, 6) NOT NULL CHECK (price >= 0 AND price <= 1),
    quantity NUMERIC(20, 8) NOT NULL CHECK (quantity >= 0),
    fee NUMERIC(12, 2) NOT NULL CHECK (fee >= 0),
    executed_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX trades_connector_external_trade_id_idx
    ON trades (connector_name, external_trade_id);

CREATE INDEX trades_order_executed_at_idx
    ON trades (order_id, executed_at DESC);

CREATE INDEX trades_market_executed_at_idx
    ON trades (market_id, executed_at DESC);

CREATE TABLE positions (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL REFERENCES markets (id) ON DELETE RESTRICT,
    connector_name TEXT NOT NULL,
    account_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    net_quantity NUMERIC(20, 8) NOT NULL CHECK (net_quantity >= 0),
    avg_cost NUMERIC(12, 6) NOT NULL CHECK (avg_cost >= 0 AND avg_cost <= 1),
    mark_price NUMERIC(12, 6) NOT NULL CHECK (mark_price >= 0 AND mark_price <= 1),
    unrealized_pnl NUMERIC(14, 2) NOT NULL,
    realized_pnl NUMERIC(14, 2) NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL,
    version BIGINT NOT NULL
);

CREATE UNIQUE INDEX positions_connector_account_market_side_idx
    ON positions (connector_name, account_id, market_id, side);

CREATE INDEX positions_market_updated_at_idx
    ON positions (market_id, updated_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0010_market_connector_refs.sql
-------------------------------------------------------------------------------

ALTER TABLE markets
    ADD COLUMN polymarket_condition_id TEXT,
    ADD COLUMN polymarket_yes_asset_id TEXT,
    ADD COLUMN polymarket_no_asset_id TEXT;

CREATE UNIQUE INDEX markets_polymarket_condition_id_uidx
    ON markets (polymarket_condition_id)
    WHERE polymarket_condition_id IS NOT NULL;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0011_news_ingestion.sql
-------------------------------------------------------------------------------

ALTER TABLE raw_events
  ADD COLUMN IF NOT EXISTS source_type TEXT,
  ADD COLUMN IF NOT EXISTS external_id TEXT,
  ADD COLUMN IF NOT EXISTS title TEXT,
  ADD COLUMN IF NOT EXISTS url TEXT,
  ADD COLUMN IF NOT EXISTS author TEXT,
  ADD COLUMN IF NOT EXISTS published_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS event_time TIMESTAMPTZ;

ALTER TABLE raw_events
  ADD CONSTRAINT raw_events_source_type_chk
  CHECK (
    source_type IS NULL
    OR source_type IN ('news', 'social', 'official', 'calendar', 'market')
  );

CREATE UNIQUE INDEX IF NOT EXISTS raw_events_source_external_id_uidx
  ON raw_events (source, external_id)
  WHERE external_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS raw_events_source_url_uidx
  ON raw_events (source, url)
  WHERE url IS NOT NULL;

CREATE INDEX IF NOT EXISTS raw_events_source_type_ingested_at_idx
  ON raw_events (source_type, ingested_at DESC);

CREATE INDEX IF NOT EXISTS raw_events_published_at_idx
  ON raw_events (published_at DESC)
  WHERE published_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS news_source_health (
  source TEXT PRIMARY KEY,
  source_type TEXT NOT NULL CHECK (source_type IN ('news', 'social', 'official', 'calendar', 'market')),
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  reliability NUMERIC(12, 6) NOT NULL CHECK (reliability >= 0 AND reliability <= 1),
  last_success_at TIMESTAMPTZ,
  last_error_at TIMESTAMPTZ,
  consecutive_failures BIGINT NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0),
  items_fetched BIGINT NOT NULL DEFAULT 0 CHECK (items_fetched >= 0),
  items_inserted BIGINT NOT NULL DEFAULT 0 CHECK (items_inserted >= 0),
  items_deduped BIGINT NOT NULL DEFAULT 0 CHECK (items_deduped >= 0),
  health_score NUMERIC(12, 6) NOT NULL DEFAULT 1 CHECK (health_score >= 0 AND health_score <= 1),
  last_error TEXT,
  updated_at TIMESTAMPTZ NOT NULL,
  trace_id TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS news_source_health_updated_at_idx
  ON news_source_health (updated_at DESC);

CREATE INDEX IF NOT EXISTS news_source_health_score_idx
  ON news_source_health (health_score, updated_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0012_news_source_health_list_index.sql
-------------------------------------------------------------------------------

CREATE INDEX IF NOT EXISTS news_source_health_source_type_updated_at_idx
  ON news_source_health (source_type, updated_at DESC, source);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0013_arbitrage_radar.sql
-------------------------------------------------------------------------------

CREATE TABLE arbitrage_scans (
    id TEXT PRIMARY KEY,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    market_count INTEGER NOT NULL DEFAULT 0 CHECK (market_count >= 0),
    snapshot_count INTEGER NOT NULL DEFAULT 0 CHECK (snapshot_count >= 0),
    opportunity_count INTEGER NOT NULL DEFAULT 0 CHECK (opportunity_count >= 0),
    scanner_version TEXT NOT NULL,
    metadata_json JSONB NOT NULL CHECK (jsonb_typeof(metadata_json) = 'object'),
    trace_id TEXT NOT NULL
);

CREATE INDEX arbitrage_scans_started_at_idx
    ON arbitrage_scans (started_at DESC);

CREATE TABLE market_book_snapshots (
    id TEXT PRIMARY KEY,
    scan_id TEXT NOT NULL REFERENCES arbitrage_scans(id) ON DELETE CASCADE,
    connector_name TEXT NOT NULL,
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    yes_asset_id TEXT,
    no_asset_id TEXT,
    yes_bid NUMERIC(12, 6) CHECK (yes_bid IS NULL OR (yes_bid >= 0 AND yes_bid <= 1)),
    yes_ask NUMERIC(12, 6) CHECK (yes_ask IS NULL OR (yes_ask >= 0 AND yes_ask <= 1)),
    yes_bid_size NUMERIC(24, 8) NOT NULL CHECK (yes_bid_size >= 0),
    yes_ask_size NUMERIC(24, 8) NOT NULL CHECK (yes_ask_size >= 0),
    no_bid NUMERIC(12, 6) CHECK (no_bid IS NULL OR (no_bid >= 0 AND no_bid <= 1)),
    no_ask NUMERIC(12, 6) CHECK (no_ask IS NULL OR (no_ask >= 0 AND no_ask <= 1)),
    no_bid_size NUMERIC(24, 8) NOT NULL CHECK (no_bid_size >= 0),
    no_ask_size NUMERIC(24, 8) NOT NULL CHECK (no_ask_size >= 0),
    observed_at TIMESTAMPTZ NOT NULL,
    raw_payload_json JSONB NOT NULL CHECK (jsonb_typeof(raw_payload_json) = 'object'),
    trace_id TEXT NOT NULL
);

CREATE INDEX market_book_snapshots_scan_observed_at_idx
    ON market_book_snapshots (scan_id, observed_at DESC);

CREATE INDEX market_book_snapshots_market_observed_at_idx
    ON market_book_snapshots (market_id, observed_at DESC);

CREATE TABLE arbitrage_opportunities (
    id TEXT PRIMARY KEY,
    scan_id TEXT NOT NULL REFERENCES arbitrage_scans(id) ON DELETE CASCADE,
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    opportunity_type TEXT NOT NULL CHECK (
        opportunity_type IN ('binary_buy_both', 'binary_sell_both')
    ),
    status TEXT NOT NULL CHECK (status IN ('observed', 'expired', 'repeated')),
    gross_edge NUMERIC(12, 6) NOT NULL CHECK (gross_edge >= -1 AND gross_edge <= 1),
    price_sum NUMERIC(12, 6) NOT NULL CHECK (price_sum >= 0 AND price_sum <= 2),
    capacity NUMERIC(24, 8) NOT NULL CHECK (capacity >= 0),
    yes_price NUMERIC(12, 6) NOT NULL CHECK (yes_price >= 0 AND yes_price <= 1),
    no_price NUMERIC(12, 6) NOT NULL CHECK (no_price >= 0 AND no_price <= 1),
    yes_size NUMERIC(24, 8) NOT NULL CHECK (yes_size >= 0),
    no_size NUMERIC(24, 8) NOT NULL CHECK (no_size >= 0),
    observed_at TIMESTAMPTZ NOT NULL,
    reason_codes_json JSONB NOT NULL CHECK (jsonb_typeof(reason_codes_json) = 'array'),
    analysis_payload_json JSONB NOT NULL CHECK (jsonb_typeof(analysis_payload_json) = 'object'),
    trace_id TEXT NOT NULL
);

CREATE INDEX arbitrage_opportunities_observed_at_idx
    ON arbitrage_opportunities (observed_at DESC);

CREATE INDEX arbitrage_opportunities_market_observed_at_idx
    ON arbitrage_opportunities (market_id, observed_at DESC);

CREATE INDEX arbitrage_opportunities_type_observed_at_idx
    ON arbitrage_opportunities (opportunity_type, observed_at DESC);

CREATE TABLE arbitrage_analysis_runs (
    id TEXT PRIMARY KEY,
    generated_at TIMESTAMPTZ NOT NULL,
    lookback_hours INTEGER NOT NULL CHECK (lookback_hours > 0),
    opportunity_count INTEGER NOT NULL CHECK (opportunity_count >= 0),
    market_count INTEGER NOT NULL CHECK (market_count >= 0),
    summary_payload_json JSONB NOT NULL CHECK (jsonb_typeof(summary_payload_json) = 'object'),
    trace_id TEXT NOT NULL
);

CREATE INDEX arbitrage_analysis_runs_generated_at_idx
    ON arbitrage_analysis_runs (generated_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0014_arbitrage_validation_events.sql
-------------------------------------------------------------------------------

CREATE TABLE arbitrage_opportunity_validations (
    id TEXT PRIMARY KEY,
    opportunity_id TEXT NOT NULL REFERENCES arbitrage_opportunities(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (
        status IN (
            'unvalidated',
            'valid',
            'stale_book',
            'insufficient_depth',
            'price_moved',
            'fees_exceed_edge',
            'below_threshold',
            'invalid_market',
            'error'
        )
    ),
    gross_edge NUMERIC(12, 6) NOT NULL CHECK (gross_edge >= -1 AND gross_edge <= 1),
    net_edge NUMERIC(12, 6) NOT NULL CHECK (net_edge >= -1 AND net_edge <= 1),
    fee_estimate NUMERIC(12, 6) NOT NULL CHECK (fee_estimate >= -1 AND fee_estimate <= 1),
    slippage_buffer NUMERIC(12, 6) NOT NULL CHECK (slippage_buffer >= -1 AND slippage_buffer <= 1),
    validated_capacity NUMERIC(24, 8) NOT NULL CHECK (validated_capacity >= 0),
    book_age_ms BIGINT NOT NULL CHECK (book_age_ms >= 0),
    reason_codes_json JSONB NOT NULL CHECK (jsonb_typeof(reason_codes_json) = 'array'),
    validation_payload_json JSONB NOT NULL CHECK (jsonb_typeof(validation_payload_json) = 'object'),
    validated_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL
);

CREATE INDEX arbitrage_opportunity_validations_opportunity_validated_at_idx
    ON arbitrage_opportunity_validations (opportunity_id, validated_at DESC);

CREATE INDEX arbitrage_opportunity_validations_status_validated_at_idx
    ON arbitrage_opportunity_validations (status, validated_at DESC);

CREATE TABLE arbitrage_events (
    sequence BIGSERIAL PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'arbitrage.scan.started',
            'arbitrage.scan.completed',
            'arbitrage.opportunity.observed',
            'arbitrage.opportunity.repeated',
            'arbitrage.opportunity.expired',
            'arbitrage.validation.passed',
            'arbitrage.validation.failed',
            'arbitrage.analysis.generated'
        )
    ),
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    payload_json JSONB NOT NULL CHECK (jsonb_typeof(payload_json) = 'object'),
    occurred_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL
);

CREATE INDEX arbitrage_events_occurred_at_idx
    ON arbitrage_events (occurred_at DESC);

CREATE INDEX arbitrage_events_resource_idx
    ON arbitrage_events (resource_type, resource_id, sequence DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0015_reward_bot.sql
-------------------------------------------------------------------------------

CREATE TABLE reward_bot_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE reward_markets (
    condition_id TEXT PRIMARY KEY,
    question TEXT NOT NULL,
    market_slug TEXT NOT NULL,
    event_slug TEXT NOT NULL DEFAULT '',
    image TEXT NOT NULL DEFAULT '',
    rewards_max_spread NUMERIC(12, 6) NOT NULL CHECK (rewards_max_spread >= 0),
    rewards_min_size NUMERIC(24, 8) NOT NULL CHECK (rewards_min_size >= 0),
    total_daily_rate NUMERIC(14, 4) NOT NULL CHECK (total_daily_rate >= 0),
    tokens_json JSONB NOT NULL CHECK (jsonb_typeof(tokens_json) = 'array'),
    active BOOLEAN NOT NULL DEFAULT true,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX reward_markets_active_rate_idx
    ON reward_markets (active, total_daily_rate DESC, updated_at DESC);

CREATE TABLE reward_quote_plans (
    condition_id TEXT PRIMARY KEY REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    score NUMERIC(10, 4) NOT NULL CHECK (score >= 0),
    eligible BOOLEAN NOT NULL DEFAULT false,
    reason TEXT NOT NULL,
    quote_plan_json JSONB NOT NULL CHECK (jsonb_typeof(quote_plan_json) = 'object'),
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX reward_quote_plans_eligible_score_idx
    ON reward_quote_plans (eligible, score DESC, updated_at DESC);

CREATE TABLE reward_managed_orders (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 6) NOT NULL CHECK (price > 0 AND price < 1),
    size NUMERIC(24, 8) NOT NULL CHECK (size > 0),
    external_order_id TEXT,
    status TEXT NOT NULL CHECK (
        status IN ('planned', 'open', 'cancelled', 'filled', 'exit_pending', 'error')
    ),
    scoring BOOLEAN NOT NULL DEFAULT false,
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL
);

CREATE UNIQUE INDEX reward_managed_orders_external_order_id_idx
    ON reward_managed_orders (external_order_id)
    WHERE external_order_id IS NOT NULL;

CREATE INDEX reward_managed_orders_account_status_idx
    ON reward_managed_orders (account_id, status, updated_at DESC);

CREATE INDEX reward_managed_orders_condition_status_idx
    ON reward_managed_orders (condition_id, status, updated_at DESC);

CREATE TABLE reward_positions (
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    size NUMERIC(24, 8) NOT NULL DEFAULT 0,
    avg_price NUMERIC(12, 6) NOT NULL DEFAULT 0,
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (account_id, token_id)
);

CREATE INDEX reward_positions_account_condition_idx
    ON reward_positions (account_id, condition_id);

CREATE TABLE reward_risk_events (
    id TEXT PRIMARY KEY,
    account_id TEXT,
    condition_id TEXT,
    external_order_id TEXT,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    message TEXT NOT NULL,
    metadata_json JSONB NOT NULL CHECK (jsonb_typeof(metadata_json) = 'object'),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX reward_risk_events_created_at_idx
    ON reward_risk_events (created_at DESC);

CREATE INDEX reward_risk_events_account_created_at_idx
    ON reward_risk_events (account_id, created_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0016_runtime_config.sql
-------------------------------------------------------------------------------

CREATE TABLE runtime_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0017_market_slug.sql
-------------------------------------------------------------------------------

ALTER TABLE markets ADD COLUMN slug TEXT;
CREATE INDEX markets_slug_idx ON markets (slug) WHERE slug IS NOT NULL;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0018_market_categories.sql
-------------------------------------------------------------------------------

CREATE TABLE market_categories (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO market_categories (id, label, sort_order) VALUES
    ('sports', 'Sports', 1),
    ('politics', 'Politics', 2),
    ('crypto', 'Crypto', 3),
    ('esports', 'Esports', 4),
    ('finance', 'Finance', 5),
    ('geopolitics', 'Geopolitics', 6),
    ('tech', 'Tech', 7),
    ('culture', 'Culture', 8),
    ('economy', 'Economy', 9),
    ('weather', 'Weather', 10),
    ('pop_culture', 'Pop Culture', 11),
    ('ai', 'AI', 12),
    ('elections', 'Elections', 13);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0019_reward_simulation.sql
-------------------------------------------------------------------------------

-- Stateful rewards market-making simulation: order fills, fund-pool ledger.

ALTER TABLE reward_managed_orders
    ADD COLUMN filled_size NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (filled_size >= 0),
    ADD COLUMN reward_earned NUMERIC(14, 4) NOT NULL DEFAULT 0,
    ADD COLUMN last_scored_at TIMESTAMPTZ;

CREATE TABLE reward_fills (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 6) NOT NULL CHECK (price > 0 AND price < 1),
    size NUMERIC(24, 8) NOT NULL CHECK (size > 0),
    notional_usd NUMERIC(18, 4) NOT NULL CHECK (notional_usd >= 0),
    role TEXT NOT NULL CHECK (role IN ('maker', 'taker')),
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    reason TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX reward_fills_account_created_at_idx
    ON reward_fills (account_id, created_at DESC);

CREATE INDEX reward_fills_condition_created_at_idx
    ON reward_fills (condition_id, created_at DESC);

CREATE TABLE reward_account_state (
    account_id TEXT PRIMARY KEY,
    capital_usd NUMERIC(18, 4) NOT NULL CHECK (capital_usd >= 0),
    available_usd NUMERIC(18, 4) NOT NULL,
    reserved_usd NUMERIC(18, 4) NOT NULL DEFAULT 0,
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    reward_earned_usd NUMERIC(14, 4) NOT NULL DEFAULT 0,
    fees_paid NUMERIC(14, 4) NOT NULL DEFAULT 0,
    tick_index BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL
);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0020_copy_trading.sql
-------------------------------------------------------------------------------

-- Smart-money copy trading: tracked wallets + wallet analysis stats, detected
-- source trades, mirrored copy orders/positions, a simulated fund-pool ledger,
-- and an activity/risk event log. Mirrors the reward_* simulation tables.

CREATE TABLE copytrade_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE copytrade_wallets (
    address TEXT PRIMARY KEY,
    label TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL CHECK (status IN ('active', 'paused')),
    sizing_override TEXT,
    max_exposure_override NUMERIC(18, 4),
    trades_window INTEGER NOT NULL DEFAULT 0,
    volume_window_usd NUMERIC(18, 4) NOT NULL DEFAULT 0,
    realized_pnl_window NUMERIC(18, 4) NOT NULL DEFAULT 0,
    win_rate NUMERIC(6, 4) NOT NULL DEFAULT 0,
    roi NUMERIC(12, 4) NOT NULL DEFAULT 0,
    avg_trade_usd NUMERIC(18, 4) NOT NULL DEFAULT 0,
    markets_traded INTEGER NOT NULL DEFAULT 0,
    last_active_at TIMESTAMPTZ,
    last_analyzed_at TIMESTAMPTZ,
    added_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX copytrade_wallets_status_idx
    ON copytrade_wallets (status, updated_at DESC);

-- One detected trade from a tracked wallet. `id` is deterministic over the
-- wallet/tx/token/side/timestamp so re-scans dedupe via ON CONFLICT (id).
CREATE TABLE copytrade_source_trades (
    id TEXT PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL DEFAULT '',
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 6) NOT NULL CHECK (price >= 0),
    size NUMERIC(24, 8) NOT NULL CHECK (size >= 0),
    usd_size NUMERIC(18, 4) NOT NULL CHECK (usd_size >= 0),
    title TEXT NOT NULL DEFAULT '',
    source_tx_hash TEXT NOT NULL DEFAULT '',
    source_timestamp TIMESTAMPTZ NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    copied BOOLEAN NOT NULL DEFAULT false,
    decision_reason TEXT NOT NULL DEFAULT ''
);

CREATE INDEX copytrade_source_trades_observed_idx
    ON copytrade_source_trades (observed_at DESC);

CREATE INDEX copytrade_source_trades_wallet_idx
    ON copytrade_source_trades (wallet_address, source_timestamp DESC);

CREATE TABLE copytrade_orders (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    wallet_address TEXT NOT NULL,
    source_trade_id TEXT NOT NULL DEFAULT '',
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL DEFAULT '',
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 6) NOT NULL CHECK (price > 0 AND price < 1),
    size NUMERIC(24, 8) NOT NULL CHECK (size > 0),
    notional_usd NUMERIC(18, 4) NOT NULL CHECK (notional_usd >= 0),
    external_order_id TEXT,
    status TEXT NOT NULL CHECK (
        status IN ('planned', 'open', 'filled', 'cancelled', 'error')
    ),
    reason TEXT NOT NULL DEFAULT '',
    filled_size NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (filled_size >= 0),
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL DEFAULT ''
);

CREATE UNIQUE INDEX copytrade_orders_external_order_id_idx
    ON copytrade_orders (external_order_id)
    WHERE external_order_id IS NOT NULL;

CREATE INDEX copytrade_orders_account_status_idx
    ON copytrade_orders (account_id, status, updated_at DESC);

CREATE INDEX copytrade_orders_wallet_idx
    ON copytrade_orders (wallet_address, updated_at DESC);

CREATE TABLE copytrade_positions (
    account_id TEXT NOT NULL,
    wallet_address TEXT NOT NULL DEFAULT '',
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    outcome TEXT NOT NULL DEFAULT '',
    size NUMERIC(24, 8) NOT NULL DEFAULT 0,
    avg_price NUMERIC(12, 6) NOT NULL DEFAULT 0,
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (account_id, token_id)
);

CREATE INDEX copytrade_positions_account_condition_idx
    ON copytrade_positions (account_id, condition_id);

CREATE TABLE copytrade_account_state (
    account_id TEXT PRIMARY KEY,
    capital_usd NUMERIC(18, 4) NOT NULL CHECK (capital_usd >= 0),
    available_usd NUMERIC(18, 4) NOT NULL,
    reserved_usd NUMERIC(18, 4) NOT NULL DEFAULT 0,
    realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0,
    fees_paid NUMERIC(14, 4) NOT NULL DEFAULT 0,
    tick_index BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE copytrade_events (
    id TEXT PRIMARY KEY,
    wallet_address TEXT,
    condition_id TEXT,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    message TEXT NOT NULL,
    metadata_json JSONB NOT NULL CHECK (jsonb_typeof(metadata_json) = 'object'),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX copytrade_events_created_at_idx
    ON copytrade_events (created_at DESC);

CREATE INDEX copytrade_events_wallet_created_at_idx
    ON copytrade_events (wallet_address, created_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0021_copytrade_daily_pnl.sql
-------------------------------------------------------------------------------

-- Add daily_realized_pnl column to copytrade_account_state for per-day PnL tracking.
-- This field resets to zero when the UTC date rolls over (handled by the engine).
-- Used by the daily_loss_limit_usd risk check.

ALTER TABLE copytrade_account_state
    ADD COLUMN daily_realized_pnl NUMERIC(14, 4) NOT NULL DEFAULT 0;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0022_reward_bot_control_commands.sql
-------------------------------------------------------------------------------

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


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0023_copytrade_control_commands.sql
-------------------------------------------------------------------------------

CREATE TABLE copytrade_control_commands (
    id TEXT PRIMARY KEY,
    action TEXT NOT NULL CHECK (action IN ('run_once', 'analyze_wallets', 'cancel_all', 'reset')),
    account_id TEXT,
    reason TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    requested_at TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    trace_id TEXT,
    error TEXT
);

CREATE INDEX copytrade_control_commands_pending_idx
    ON copytrade_control_commands (status, requested_at)
    WHERE status = 'pending';

CREATE INDEX copytrade_control_commands_recent_idx
    ON copytrade_control_commands (requested_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0024_reward_markets_active_index.sql
-------------------------------------------------------------------------------

-- Index for reward_markets snapshot query: active=true ORDER BY total_daily_rate DESC, updated_at DESC
CREATE INDEX IF NOT EXISTS idx_reward_markets_active_daily_rate
    ON reward_markets (active, total_daily_rate DESC, updated_at DESC)
    WHERE active = true;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0025_markets_active_volume_index.sql
-------------------------------------------------------------------------------

-- Supports rewards candidate selection by active/tradable market activity.
CREATE INDEX IF NOT EXISTS idx_markets_open_tradable_volume
    ON markets (status, tradability_status, volume_24h DESC, updated_at DESC)
    WHERE status = 'open' AND tradability_status = 'tradable';


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0026_reward_control_running_lease_index.sql
-------------------------------------------------------------------------------

CREATE INDEX IF NOT EXISTS idx_reward_control_commands_running_started_at
ON reward_control_commands (started_at, requested_at)
WHERE status = 'running';


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0027_remove_paper_trade_manual_confirm.sql
-------------------------------------------------------------------------------

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


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0028_reward_positions_external_inventory.sql
-------------------------------------------------------------------------------

-- Rewards account reconciliation stores the complete Polymarket wallet inventory,
-- including positions in markets that are not currently in the rewards catalog.
ALTER TABLE reward_positions
    DROP CONSTRAINT IF EXISTS reward_positions_condition_id_fkey;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0029_reward_account_wallet_address.sql
-------------------------------------------------------------------------------

-- Add wallet_address to reward_account_state for displaying the Polymarket wallet
-- address configured in the worker service.
ALTER TABLE reward_account_state ADD COLUMN IF NOT EXISTS wallet_address TEXT;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0030_rewards_snapshot_indexes.sql
-------------------------------------------------------------------------------

-- Add indexes for rewards snapshot queries that previously required full table scans.
--
-- pg_trgm GIN indexes:  reward_managed_orders search uses ILIKE '%val%' on
--   outcome, condition_id, and token_id.  Leading-wildcard ILIKE cannot use
--   B-tree indexes.  pg_trgm GIN indexes support these patterns efficiently.
--
-- reward_quote_plans JSON text search uses quote_plan_json::text ILIKE '%val%'.
--   An expression GIN index on the cast allows pg_trgm to serve the query.
--
-- reward_fills_created_at_idx:  fallback for unfiltered ORDER BY created_at DESC.
--   The primary query path now filters by account_id and uses the existing
--   (account_id, created_at DESC) composite index, but this standalone index
--   covers any code path that queries fills without an account_id filter.
--
-- reward_positions_updated_at_idx:  partial index for WHERE size <> 0
--   ORDER BY updated_at DESC.  The primary query path now filters by account_id
--   (using the PK prefix), but this covers any code path querying positions
--   across accounts.

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS reward_managed_orders_outcome_trgm_idx
    ON reward_managed_orders USING GIN (outcome gin_trgm_ops);

CREATE INDEX IF NOT EXISTS reward_managed_orders_condition_id_trgm_idx
    ON reward_managed_orders USING GIN (condition_id gin_trgm_ops);

CREATE INDEX IF NOT EXISTS reward_managed_orders_token_id_trgm_idx
    ON reward_managed_orders USING GIN (token_id gin_trgm_ops);

CREATE INDEX IF NOT EXISTS reward_quote_plans_json_trgm_idx
    ON reward_quote_plans USING GIN ((quote_plan_json::text) gin_trgm_ops);

CREATE INDEX IF NOT EXISTS reward_fills_created_at_idx
    ON reward_fills (created_at DESC);

CREATE INDEX IF NOT EXISTS reward_positions_updated_at_idx
    ON reward_positions (updated_at DESC)
    WHERE size <> 0;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0031_worker_query_indexes.sql
-------------------------------------------------------------------------------

-- Add indexes for worker query paths that previously lacked covering indexes.
--
-- orders_connector_status_updated_at_idx:
--   Worker's list_orders query filters on connector_name + status and sorts by
--   updated_at DESC.  The existing (signal_id, status, updated_at) index is
--   never used because workers always pass signal_id = NULL.  The existing
--   (connector_name, external_order_id) index does not cover the status filter
--   or updated_at sort.  This composite index covers all three worker paths:
--   drain-execution-queue order polling, consume-polymarket-user-events market
--   collection, and register-orderbook-tokens active order lookup.
--
-- raw_events_event_time_idx:
--   promote-news-events queries raw_events ORDER BY event_time DESC.  The
--   existing indexes cover published_at and (source_type, ingested_at) but not
--   event_time, causing a sequential scan + sort as the table grows.
--
-- copytrade_source_trades_source_timestamp_idx:
--   list_source_trades queries ORDER BY source_timestamp DESC without a wallet
--   filter.  The existing (wallet_address, source_timestamp DESC) composite
--   index only helps when wallet_address is specified.  This standalone index
--   covers the unfiltered sort used by the worker.

CREATE INDEX IF NOT EXISTS orders_connector_status_updated_at_idx
    ON orders (connector_name, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS raw_events_event_time_idx
    ON raw_events (event_time DESC);

CREATE INDEX IF NOT EXISTS copytrade_source_trades_source_timestamp_idx
    ON copytrade_source_trades (source_timestamp DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0032_reward_worker_heartbeats.sql
-------------------------------------------------------------------------------

CREATE TABLE reward_worker_heartbeats (
    account_id TEXT PRIMARY KEY,
    observed_at TIMESTAMPTZ NOT NULL
);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0033_reward_candidate_filter.sql
-------------------------------------------------------------------------------

-- Supports the reward candidate market query with pushed-down filters.
-- The partial index covers active reward markets with valid tokens and spread,
-- enabling PostgreSQL to skip rows that fail these checks during the candidate scan.
CREATE INDEX IF NOT EXISTS idx_reward_candidates_filtered
    ON reward_markets (total_daily_rate DESC, updated_at DESC)
    WHERE active = true
      AND rewards_max_spread > 0
      AND jsonb_array_length(tokens_json) >= 2;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0034_reward_account_external_buy_notional.sql
-------------------------------------------------------------------------------

-- 0034_reward_account_external_buy_notional.sql
-- Track total notional of all active buy orders on Polymarket (bot-managed +
-- external) for CLOB balance pre-check during order placement.

ALTER TABLE reward_account_state
    ADD COLUMN IF NOT EXISTS external_buy_notional NUMERIC(18, 4) NOT NULL DEFAULT 0;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0035_auto_cancel_not_found_orders.sql
-------------------------------------------------------------------------------

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

-- Reset transient rejection errors (HTTP 425, "order manager not ready") back
-- to Planned so the worker retries them on the next cycle instead of leaving
-- them permanently stuck as Error.

UPDATE reward_managed_orders
SET
    status = 'planned',
    scoring = true,
    reason = 'reset from error by migration 0033; transient rejection: ' || reason,
    updated_at = now()
WHERE status = 'error'
  AND reason LIKE '%order manager not ready%';


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0036_restore_not_found_reconciliation.sql
-------------------------------------------------------------------------------

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


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0037_reward_market_quality.sql
-------------------------------------------------------------------------------

ALTER TABLE markets
    ADD COLUMN liquidity_usd NUMERIC(18, 2) NOT NULL DEFAULT 0
        CHECK (liquidity_usd >= 0),
    ADD COLUMN end_at TIMESTAMPTZ,
    ADD COLUMN synced_at TIMESTAMPTZ NOT NULL DEFAULT now();

CREATE INDEX idx_markets_reward_quality
    ON markets (
        status,
        tradability_status,
        ambiguity_level,
        liquidity_usd DESC,
        volume_24h DESC,
        end_at DESC,
        synced_at DESC
    )
    WHERE polymarket_condition_id IS NOT NULL;

-- Runtime versions before this migration could locally mark an unresolved
-- external order as cancelled after a timeout. Restore those rows to a locked
-- reconciliation state so a potentially live or filled exchange order cannot
-- be forgotten and replaced with duplicate exposure.
UPDATE reward_managed_orders
SET status = CASE
        WHEN external_order_id IS NULL THEN 'planned'
        ELSE 'open'
    END,
    scoring = false,
    reason = CASE
        WHEN external_order_id IS NULL THEN
            'live submission attempted; live submission result unknown; manual reconciliation required; restored by migration 0037'
        ELSE
            'awaiting final reconciliation; stale local auto-cancel restored by migration 0037: '
                || external_order_id
    END,
    updated_at = now()
WHERE status = 'cancelled'
  AND reason LIKE 'auto-cancelled stale order:%';


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0038_reward_market_advisories.sql
-------------------------------------------------------------------------------

CREATE TABLE reward_market_advisories (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('openai', 'anthropic')),
    request_format TEXT NOT NULL CHECK (
        request_format IN (
            'openai_responses',
            'openai_chat_completions',
            'anthropic_messages'
        )
    ),
    model TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    suitability TEXT NOT NULL CHECK (suitability IN ('allow', 'avoid', 'watch')),
    quote_mode TEXT NOT NULL CHECK (quote_mode IN ('double', 'single_yes', 'single_no', 'none')),
    exit_policy TEXT NOT NULL,
    confidence NUMERIC(5, 4) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    reasons_json JSONB NOT NULL CHECK (jsonb_typeof(reasons_json) = 'array'),
    metrics_json JSONB NOT NULL CHECK (jsonb_typeof(metrics_json) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_reward_market_advisories_condition_expires
    ON reward_market_advisories (condition_id, expires_at DESC);

CREATE INDEX idx_reward_market_advisories_provider_input
    ON reward_market_advisories (provider, request_format, model, input_hash);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0039_reward_market_info_risks.sql
-------------------------------------------------------------------------------

CREATE TABLE reward_market_info_risks (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
  provider TEXT NOT NULL CHECK (provider IN ('openai', 'anthropic')),
  request_format TEXT NOT NULL CHECK (
    request_format IN ('openai_responses', 'openai_chat_completions', 'anthropic_messages')
  ),
  model TEXT NOT NULL,
  query_hash TEXT NOT NULL,
  input_hash TEXT NOT NULL,
  risk_level TEXT NOT NULL CHECK (
    risk_level IN ('low', 'medium', 'high', 'critical', 'unknown')
  ),
  risk_type TEXT NOT NULL CHECK (
    risk_type IN (
      'imminent_resolution',
      'breaking_news',
      'scheduled_event',
      'official_result',
      'rumor',
      'stale',
      'none',
      'unknown'
    )
  ),
  directional_risk TEXT NOT NULL CHECK (directional_risk IN ('yes', 'no', 'unclear')),
  resolution_imminent BOOLEAN NOT NULL DEFAULT false,
  expected_event_at TIMESTAMPTZ,
  confidence NUMERIC(5,4) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
  summary TEXT NOT NULL,
  sources_json JSONB NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(sources_json) = 'array'),
  metrics_json JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metrics_json) = 'object'),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_reward_market_info_risks_condition_expires
  ON reward_market_info_risks (condition_id, expires_at DESC);

CREATE INDEX idx_reward_market_info_risks_level_expires
  ON reward_market_info_risks (risk_level, expires_at DESC);

CREATE INDEX idx_reward_market_info_risks_request_cache
  ON reward_market_info_risks (
    condition_id,
    provider,
    request_format,
    model,
    input_hash,
    expires_at DESC
  );


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0040_markets_quality_index_no_synced_at.sql
-------------------------------------------------------------------------------

-- Keep high-frequency market freshness refreshes out of the rewards quality
-- index. Rewards queries still filter synced_at from the heap row after using
-- the stable market quality columns and reward_markets join keys.
DROP INDEX IF EXISTS idx_markets_reward_quality;

CREATE INDEX idx_markets_reward_quality
    ON markets (
        status,
        tradability_status,
        ambiguity_level,
        liquidity_usd DESC,
        volume_24h DESC,
        end_at DESC
    )
    WHERE polymarket_condition_id IS NOT NULL;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0041_market_asset_id_lookup_indexes.sql
-------------------------------------------------------------------------------

-- Speed up orderbook priority sync token -> condition lookups.
CREATE INDEX IF NOT EXISTS idx_markets_polymarket_yes_asset_id
    ON markets (polymarket_yes_asset_id)
    WHERE polymarket_yes_asset_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_markets_polymarket_no_asset_id
    ON markets (polymarket_no_asset_id)
    WHERE polymarket_no_asset_id IS NOT NULL;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0042_reward_order_strategy_bucket.sql
-------------------------------------------------------------------------------

ALTER TABLE reward_managed_orders
    ADD COLUMN strategy_bucket TEXT NOT NULL DEFAULT 'standard'
        CHECK (strategy_bucket IN ('standard', 'low_competition', 'none'));

CREATE INDEX reward_managed_orders_bucket_status_idx
    ON reward_managed_orders (strategy_bucket, account_id, status, updated_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0043_reward_low_competition_observations.sql
-------------------------------------------------------------------------------

CREATE TABLE reward_low_competition_observations (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    market_slug TEXT NOT NULL,
    question TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('observe', 'enforce')),
    planned_notional_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    competition_probe_notional_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    qualified_competition_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    competition_share_bps NUMERIC(20,8) NOT NULL DEFAULT 0,
    competition_multiple NUMERIC(20,8) NOT NULL DEFAULT 0,
    estimated_reward_per_100_usd_day NUMERIC(20,8) NOT NULL DEFAULT 0,
    competition_density NUMERIC(20,8) NOT NULL DEFAULT 0,
    account_effective_available_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    low_competition_open_buy_notional_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    low_competition_open_buy_notional_usd_after_plan NUMERIC(20,8) NOT NULL DEFAULT 0,
    condition_buy_notional_usd_after_plan NUMERIC(20,8) NOT NULL DEFAULT 0,
    account_allocation_bps NUMERIC(20,8) NOT NULL DEFAULT 0,
    market_allocation_bps NUMERIC(20,8) NOT NULL DEFAULT 0,
    exit_depth_usd NUMERIC(20,8) NOT NULL DEFAULT 0,
    exit_slippage_cents NUMERIC(20,8),
    midpoint_range_cents NUMERIC(20,8),
    top_of_book_flip_count BIGINT,
    sample_count BIGINT NOT NULL DEFAULT 0,
    sample_insufficient BOOLEAN NOT NULL DEFAULT false,
    eligible_for_low_competition BOOLEAN NOT NULL DEFAULT false,
    final_eligible BOOLEAN NOT NULL DEFAULT false,
    ai_blocked BOOLEAN NOT NULL DEFAULT false,
    info_risk_blocked BOOLEAN NOT NULL DEFAULT false,
    standard_plan_overlap BOOLEAN NOT NULL DEFAULT false,
    not_low_competition BOOLEAN NOT NULL DEFAULT false,
    rejection_reasons JSONB NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(rejection_reasons) = 'array'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reward_low_competition_obs_recent_idx
    ON reward_low_competition_observations (account_id, observed_at DESC);

CREATE INDEX reward_low_competition_obs_condition_recent_idx
    ON reward_low_competition_observations (condition_id, observed_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0044_reward_market_candles.sql
-------------------------------------------------------------------------------

CREATE TABLE reward_market_candles (
    token_id TEXT NOT NULL,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    outcome TEXT NOT NULL,
    interval_sec INTEGER NOT NULL CHECK (interval_sec > 0),
    bucket_start TIMESTAMPTZ NOT NULL,
    open NUMERIC(20,8) NOT NULL CHECK (open >= 0 AND open <= 1),
    high NUMERIC(20,8) NOT NULL CHECK (high >= 0 AND high <= 1),
    low NUMERIC(20,8) NOT NULL CHECK (low >= 0 AND low <= 1),
    close NUMERIC(20,8) NOT NULL CHECK (close >= 0 AND close <= 1),
    best_bid_close NUMERIC(20,8) NOT NULL CHECK (best_bid_close >= 0 AND best_bid_close <= 1),
    best_ask_close NUMERIC(20,8) NOT NULL CHECK (best_ask_close >= 0 AND best_ask_close <= 1),
    spread_cents_close NUMERIC(20,8) NOT NULL DEFAULT 0 CHECK (spread_cents_close >= 0),
    sample_count INTEGER NOT NULL DEFAULT 1 CHECK (sample_count > 0),
    close_observed_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (token_id, interval_sec, bucket_start)
);

CREATE INDEX reward_market_candles_condition_recent_idx
    ON reward_market_candles (condition_id, interval_sec, bucket_start DESC);

CREATE INDEX reward_market_candles_token_recent_idx
    ON reward_market_candles (token_id, interval_sec, bucket_start DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0045_reward_control_command_dedupe.sql
-------------------------------------------------------------------------------

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


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0046_reward_low_competition_competition_share.sql
-------------------------------------------------------------------------------

-- Folded into reward_low_competition_observations above for empty-database
-- initialization. Existing databases receive these columns through migration
-- 0046.


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0048_reward_account_unmanaged_buy_notional.sql
-------------------------------------------------------------------------------

-- 0048_reward_account_unmanaged_buy_notional.sql
-- Snapshot-frozen notional of active buy orders on Polymarket that are NOT
-- tracked as bot-managed (true external/unknown buy occupancy). See migration
-- 0048 for the full rationale; funding precheck reads this directly so the
-- bot cancelling its own managed buys between CLOB snapshots no longer spikes
-- the external occupancy estimate.

ALTER TABLE reward_account_state
    ADD COLUMN IF NOT EXISTS unmanaged_external_buy_notional NUMERIC(18, 4) NOT NULL DEFAULT 0;


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0049_smart_money_intelligence.sql
-------------------------------------------------------------------------------

CREATE TABLE smart_money_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE smart_wallet_candidates (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    source TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'candidate'
        CHECK (status IN ('candidate', 'watch', 'tracked', 'blocked', 'rejected')),
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_analyzed_at TIMESTAMPTZ,
    promoted_at TIMESTAMPTZ,
    rejected_at TIMESTAMPTZ,
    reason TEXT,
    raw JSONB NOT NULL DEFAULT '{}',
    UNIQUE (wallet_address, source)
);

CREATE INDEX smart_wallet_candidates_status_seen_idx
    ON smart_wallet_candidates (status, last_seen_at DESC);
CREATE INDEX smart_wallet_candidates_wallet_idx
    ON smart_wallet_candidates (wallet_address);

CREATE TABLE smart_wallet_profiles (
    wallet_address TEXT PRIMARY KEY,
    trade_count BIGINT NOT NULL DEFAULT 0,
    settled_trade_count BIGINT NOT NULL DEFAULT 0,
    total_volume_usd NUMERIC NOT NULL DEFAULT 0,
    realized_pnl_usd NUMERIC NOT NULL DEFAULT 0,
    roi NUMERIC NOT NULL DEFAULT 0,
    win_rate NUMERIC NOT NULL DEFAULT 0,
    max_drawdown_usd NUMERIC NOT NULL DEFAULT 0,
    avg_trade_usd NUMERIC NOT NULL DEFAULT 0,
    median_trade_usd NUMERIC NOT NULL DEFAULT 0,
    avg_hold_secs BIGINT,
    active_days BIGINT NOT NULL DEFAULT 0,
    markets_traded BIGINT NOT NULL DEFAULT 0,
    category_concentration_score NUMERIC NOT NULL DEFAULT 0,
    market_concentration_score NUMERIC NOT NULL DEFAULT 0,
    low_liquidity_trade_ratio NUMERIC NOT NULL DEFAULT 0,
    stale_copy_window_ratio NUMERIC NOT NULL DEFAULT 0,
    last_trade_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_wallet_profiles_updated_idx
    ON smart_wallet_profiles (updated_at DESC);

CREATE TABLE smart_wallet_scores (
    wallet_address TEXT PRIMARY KEY REFERENCES smart_wallet_profiles(wallet_address) ON DELETE CASCADE,
    total_score NUMERIC NOT NULL,
    profit_score NUMERIC NOT NULL,
    consistency_score NUMERIC NOT NULL,
    risk_score NUMERIC NOT NULL,
    liquidity_score NUMERIC NOT NULL,
    recency_score NUMERIC NOT NULL,
    copyability_score NUMERIC NOT NULL,
    tier TEXT NOT NULL CHECK (tier IN ('blocked', 'candidate', 'watch', 'approved')),
    explanation JSONB NOT NULL DEFAULT '{}',
    scoring_version TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_wallet_scores_tier_score_idx
    ON smart_wallet_scores (tier, total_score DESC);

CREATE TABLE smart_wallet_trades (
    id TEXT PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    source TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    outcome TEXT,
    price NUMERIC NOT NULL CHECK (price >= 0 AND price <= 1),
    size NUMERIC NOT NULL CHECK (size >= 0),
    notional_usd NUMERIC NOT NULL CHECK (notional_usd >= 0),
    tx_hash TEXT,
    source_timestamp TIMESTAMPTZ NOT NULL,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    raw JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX smart_wallet_trades_wallet_time_idx
    ON smart_wallet_trades (wallet_address, source_timestamp DESC);
CREATE INDEX smart_wallet_trades_condition_time_idx
    ON smart_wallet_trades (condition_id, source_timestamp DESC);
CREATE INDEX smart_wallet_trades_discovered_idx
    ON smart_wallet_trades (discovered_at DESC);

CREATE TABLE smart_signals (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    source_trade_id TEXT NOT NULL REFERENCES smart_wallet_trades(id),
    wallet_address TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    source_price NUMERIC NOT NULL CHECK (source_price >= 0 AND source_price <= 1),
    current_price NUMERIC CHECK (current_price >= 0 AND current_price <= 1),
    price_slippage_cents NUMERIC,
    latency_ms BIGINT,
    source_notional_usd NUMERIC NOT NULL DEFAULT 0,
    consensus_wallet_count BIGINT NOT NULL DEFAULT 1,
    score NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'new'
        CHECK (status IN ('new', 'rejected', 'observe', 'paper', 'approval_required', 'live_ready', 'executed', 'expired')),
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_signals_status_created_idx
    ON smart_signals (status, created_at DESC);
CREATE INDEX smart_signals_condition_created_idx
    ON smart_signals (condition_id, created_at DESC);
CREATE INDEX smart_signals_wallet_created_idx
    ON smart_signals (wallet_address, created_at DESC);
CREATE INDEX smart_signals_source_trade_idx
    ON smart_signals (source_trade_id);

CREATE TABLE smart_signal_decisions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    signal_id BIGINT NOT NULL REFERENCES smart_signals(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'observe', 'reject')),
    stage TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('observe', 'paper', 'approval', 'live_guarded')),
    rejection_reason TEXT,
    risk_checks JSONB NOT NULL DEFAULT '{}',
    decided_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_signal_decisions_signal_idx
    ON smart_signal_decisions (signal_id, decided_at DESC);
CREATE INDEX smart_signal_decisions_decision_idx
    ON smart_signal_decisions (decision, decided_at DESC);

CREATE TABLE smart_wallet_advisories (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    provider TEXT NOT NULL,
    request_format TEXT NOT NULL,
    model TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    recommendation TEXT NOT NULL CHECK (recommendation IN ('allow', 'observe', 'reject')),
    confidence NUMERIC NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    risk_tags JSONB NOT NULL DEFAULT '[]',
    summary TEXT NOT NULL DEFAULT '',
    reasons JSONB NOT NULL DEFAULT '[]',
    raw_output JSONB NOT NULL DEFAULT '{}',
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_address, provider, request_format, model, input_hash)
);

CREATE INDEX smart_wallet_advisories_lookup_idx
    ON smart_wallet_advisories (wallet_address, expires_at DESC);

CREATE TABLE smart_signal_advisories (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    signal_id BIGINT NOT NULL REFERENCES smart_signals(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    request_format TEXT NOT NULL,
    model TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    recommendation TEXT NOT NULL CHECK (recommendation IN ('allow', 'observe', 'reject')),
    confidence NUMERIC NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    risk_tags JSONB NOT NULL DEFAULT '[]',
    summary TEXT NOT NULL DEFAULT '',
    reasons JSONB NOT NULL DEFAULT '[]',
    raw_output JSONB NOT NULL DEFAULT '{}',
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (signal_id, provider, request_format, model, input_hash)
);

CREATE INDEX smart_signal_advisories_signal_idx
    ON smart_signal_advisories (signal_id, expires_at DESC);

CREATE TABLE smart_paper_executions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    signal_id BIGINT NOT NULL REFERENCES smart_signals(id),
    account_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    token_id TEXT,
    planned_price NUMERIC NOT NULL,
    filled_price NUMERIC,
    size NUMERIC NOT NULL,
    notional_usd NUMERIC NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('planned', 'filled', 'expired', 'closed')),
    realized_pnl_usd NUMERIC NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_paper_executions_signal_idx
    ON smart_paper_executions (signal_id);
CREATE INDEX smart_paper_executions_account_created_idx
    ON smart_paper_executions (account_id, created_at DESC);

CREATE TABLE high_probability_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE high_probability_samples (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    sampled_at TIMESTAMPTZ NOT NULL,
    trigger_kind TEXT NOT NULL
        CHECK (trigger_kind IN ('first_touch', 'sustained', 're_entry')),
    executable_price NUMERIC NOT NULL CHECK (executable_price >= 0 AND executable_price <= 1),
    price_bucket TEXT NOT NULL,
    market_type TEXT NOT NULL,
    time_to_resolution_bucket TEXT,
    liquidity_bucket TEXT,
    spread_bucket TEXT,
    path_features JSONB NOT NULL DEFAULT '{}',
    risk_tags JSONB NOT NULL DEFAULT '[]',
    outcome TEXT NOT NULL DEFAULT 'unknown'
        CHECK (outcome IN ('win', 'loss', 'voided', 'unknown')),
    settlement_pnl NUMERIC,
    max_drawdown_cents NUMERIC,
    hold_seconds BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (condition_id, token_id, sampled_at, trigger_kind, price_bucket)
);

CREATE INDEX high_probability_samples_bucket_idx
    ON high_probability_samples (market_type, price_bucket, sampled_at DESC);
CREATE INDEX high_probability_samples_condition_idx
    ON high_probability_samples (condition_id);
CREATE INDEX high_probability_samples_outcome_idx
    ON high_probability_samples (outcome, sampled_at DESC);

CREATE TABLE high_probability_bucket_stats (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    model_version TEXT NOT NULL,
    bucket_key TEXT NOT NULL,
    bucket_dimensions JSONB NOT NULL,
    sample_count BIGINT NOT NULL CHECK (sample_count >= 0),
    win_count BIGINT NOT NULL CHECK (win_count >= 0),
    win_rate NUMERIC NOT NULL CHECK (win_rate >= 0 AND win_rate <= 1),
    fair_probability NUMERIC NOT NULL CHECK (fair_probability >= 0 AND fair_probability <= 1),
    confidence_low NUMERIC CHECK (confidence_low >= 0 AND confidence_low <= 1),
    confidence_high NUMERIC CHECK (confidence_high >= 0 AND confidence_high <= 1),
    expected_pnl NUMERIC,
    avg_max_drawdown_cents NUMERIC,
    break_70_rate NUMERIC CHECK (break_70_rate >= 0 AND break_70_rate <= 1),
    break_60_rate NUMERIC CHECK (break_60_rate >= 0 AND break_60_rate <= 1),
    break_50_rate NUMERIC CHECK (break_50_rate >= 0 AND break_50_rate <= 1),
    avg_hold_seconds BIGINT,
    recommended_max_entry_price NUMERIC
        CHECK (recommended_max_entry_price >= 0 AND recommended_max_entry_price <= 1),
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (model_version, bucket_key)
);

CREATE INDEX high_probability_bucket_stats_model_idx
    ON high_probability_bucket_stats (model_version, sample_count DESC);

CREATE TABLE high_probability_observations (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    observed_at TIMESTAMPTZ NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('observe', 'paper', 'live_guarded')),
    executable_price NUMERIC NOT NULL CHECK (executable_price >= 0 AND executable_price <= 1),
    fair_probability NUMERIC CHECK (fair_probability >= 0 AND fair_probability <= 1),
    net_edge NUMERIC,
    recommended_size_usd NUMERIC,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'reject', 'skip')),
    reasons JSONB NOT NULL DEFAULT '[]',
    model_version TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX high_probability_observations_recent_idx
    ON high_probability_observations (observed_at DESC);
CREATE INDEX high_probability_observations_condition_idx
    ON high_probability_observations (condition_id, observed_at DESC);

CREATE TABLE high_probability_market_outcomes (
    condition_id TEXT PRIMARY KEY,
    status TEXT NOT NULL
        CHECK (status IN ('unresolved', 'resolved', 'voided', 'ambiguous')),
    winning_token_id TEXT,
    resolved_at TIMESTAMPTZ,
    market_type TEXT NOT NULL DEFAULT 'unknown',
    risk_tags JSONB NOT NULL DEFAULT '[]',
    label_source TEXT NOT NULL DEFAULT 'manual',
    raw JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        status <> 'resolved'
        OR (winning_token_id IS NOT NULL AND winning_token_id <> '' AND resolved_at IS NOT NULL)
    )
);

CREATE INDEX high_probability_market_outcomes_status_idx
    ON high_probability_market_outcomes (status, resolved_at DESC);

CREATE TABLE high_probability_backtest_runs (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    run_at TIMESTAMPTZ NOT NULL,
    model_version TEXT NOT NULL,
    market_scope TEXT NOT NULL,
    sample_limit BIGINT NOT NULL CHECK (sample_limit > 0),
    train_sample_count BIGINT NOT NULL CHECK (train_sample_count >= 0),
    test_sample_count BIGINT NOT NULL CHECK (test_sample_count >= 0),
    candidate_count BIGINT NOT NULL CHECK (candidate_count >= 0),
    trade_count BIGINT NOT NULL CHECK (trade_count >= 0),
    skipped_no_bucket_count BIGINT NOT NULL CHECK (skipped_no_bucket_count >= 0),
    skipped_no_edge_count BIGINT NOT NULL CHECK (skipped_no_edge_count >= 0),
    win_trades BIGINT NOT NULL CHECK (win_trades >= 0),
    loss_trades BIGINT NOT NULL CHECK (loss_trades >= 0),
    win_rate NUMERIC CHECK (win_rate >= 0 AND win_rate <= 1),
    total_pnl NUMERIC NOT NULL,
    average_pnl NUMERIC,
    total_entry_cost NUMERIC NOT NULL CHECK (total_entry_cost >= 0),
    roi NUMERIC,
    max_drawdown NUMERIC NOT NULL CHECK (max_drawdown >= 0),
    average_entry_price NUMERIC CHECK (average_entry_price >= 0 AND average_entry_price <= 1),
    train_start_at TIMESTAMPTZ,
    train_end_at TIMESTAMPTZ,
    test_start_at TIMESTAMPTZ,
    test_end_at TIMESTAMPTZ,
    exit_rule_reports JSONB NOT NULL DEFAULT '[]',
    notes JSONB NOT NULL DEFAULT '[]',
    config_json JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX high_probability_backtest_runs_recent_idx
    ON high_probability_backtest_runs (run_at DESC);
CREATE INDEX high_probability_backtest_runs_model_idx
    ON high_probability_backtest_runs (model_version, run_at DESC);

CREATE TABLE high_probability_backtest_trades (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    run_id BIGINT NOT NULL REFERENCES high_probability_backtest_runs(id) ON DELETE CASCADE,
    sample_id BIGINT NOT NULL REFERENCES high_probability_samples(id),
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    sampled_at TIMESTAMPTZ NOT NULL,
    bucket_key TEXT NOT NULL,
    executable_price NUMERIC NOT NULL CHECK (executable_price >= 0 AND executable_price <= 1),
    fair_probability NUMERIC NOT NULL CHECK (fair_probability >= 0 AND fair_probability <= 1),
    net_edge NUMERIC NOT NULL,
    recommended_max_entry_price NUMERIC
        CHECK (recommended_max_entry_price >= 0 AND recommended_max_entry_price <= 1),
    outcome TEXT NOT NULL CHECK (outcome IN ('win', 'loss')),
    settlement_pnl NUMERIC NOT NULL,
    cumulative_pnl NUMERIC NOT NULL,
    drawdown NUMERIC NOT NULL CHECK (drawdown >= 0),
    reasons JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX high_probability_backtest_trades_run_idx
    ON high_probability_backtest_trades (run_id, sampled_at, id);
CREATE INDEX high_probability_backtest_trades_condition_idx
    ON high_probability_backtest_trades (condition_id, sampled_at DESC);

CREATE TABLE IF NOT EXISTS reward_market_event_windows (
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    event_type TEXT NOT NULL DEFAULT 'other',
    event_start_at TIMESTAMPTZ,
    event_end_at TIMESTAMPTZ,
    confidence TEXT NOT NULL CHECK (confidence IN ('low', 'medium', 'high')),
    source_url TEXT,
    source_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    notes TEXT NOT NULL DEFAULT '',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    reviewed_by TEXT,
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (condition_id, source)
);

CREATE INDEX IF NOT EXISTS reward_market_event_windows_active_idx
    ON reward_market_event_windows (
        condition_id,
        active,
        confidence,
        updated_at DESC
    );

CREATE INDEX IF NOT EXISTS reward_market_event_windows_start_idx
    ON reward_market_event_windows (event_start_at)
    WHERE active AND event_start_at IS NOT NULL;

ALTER TABLE reward_managed_orders
    ADD COLUMN strategy_profile TEXT NOT NULL DEFAULT 'standard'
        CHECK (strategy_profile IN ('standard', 'balanced_merge'));

CREATE INDEX reward_managed_orders_profile_status_idx
    ON reward_managed_orders (strategy_profile, account_id, status, updated_at DESC);

ALTER TABLE reward_quote_plans
    ADD COLUMN strategy_profile TEXT NOT NULL DEFAULT 'standard'
        CHECK (strategy_profile IN ('standard', 'balanced_merge'));

CREATE INDEX reward_quote_plans_profile_eligible_score_idx
    ON reward_quote_plans (strategy_profile, eligible, score DESC, updated_at DESC);

CREATE TABLE reward_merge_intents (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    yes_token_id TEXT NOT NULL,
    no_token_id TEXT NOT NULL,
    merge_size NUMERIC(24, 8) NOT NULL CHECK (merge_size > 0),
    yes_position_size NUMERIC(24, 8) NOT NULL CHECK (yes_position_size >= 0),
    no_position_size NUMERIC(24, 8) NOT NULL CHECK (no_position_size >= 0),
    yes_avg_price NUMERIC(12, 6) NOT NULL DEFAULT 0,
    no_avg_price NUMERIC(12, 6) NOT NULL DEFAULT 0,
    status TEXT NOT NULL CHECK (
        status IN ('pending', 'unsupported', 'submitted', 'completed', 'failed')
    ),
    reason TEXT NOT NULL,
    source_fill_id TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX reward_merge_intents_source_fill_idx
    ON reward_merge_intents (source_fill_id);

CREATE INDEX reward_merge_intents_account_condition_status_idx
    ON reward_merge_intents (account_id, condition_id, status, updated_at DESC);


-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0056_reward_managed_orders_external_inventory.sql
-------------------------------------------------------------------------------

-- External inventory exit intents may reference markets outside the current
-- rewards catalog, just like reward_positions.
ALTER TABLE reward_managed_orders
    DROP CONSTRAINT IF EXISTS reward_managed_orders_condition_id_fkey;

-------------------------------------------------------------------------------
-- Source: packages/backend/migrations/0057_reward_merge_intent_execution.sql
-------------------------------------------------------------------------------

ALTER TABLE reward_merge_intents
    ADD COLUMN tx_hash TEXT,
    ADD COLUMN submitted_at TIMESTAMPTZ,
    ADD COLUMN confirmed_at TIMESTAMPTZ,
    ADD COLUMN failed_reason TEXT,
    ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;

CREATE INDEX reward_merge_intents_executable_idx
    ON reward_merge_intents (account_id, status, updated_at ASC)
    WHERE status IN ('pending', 'unsupported');
