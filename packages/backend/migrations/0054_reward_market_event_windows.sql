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
