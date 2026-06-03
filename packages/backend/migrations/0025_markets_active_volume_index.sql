-- Supports rewards candidate selection by active/tradable market activity.
CREATE INDEX IF NOT EXISTS idx_markets_open_tradable_volume
    ON markets (status, tradability_status, volume_24h DESC, updated_at DESC)
    WHERE status = 'open' AND tradability_status = 'tradable';
