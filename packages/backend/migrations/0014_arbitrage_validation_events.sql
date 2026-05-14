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
