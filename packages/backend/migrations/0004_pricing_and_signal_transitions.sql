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
