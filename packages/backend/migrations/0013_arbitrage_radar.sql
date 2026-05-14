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
