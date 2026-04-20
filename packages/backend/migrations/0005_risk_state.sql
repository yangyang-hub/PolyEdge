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
