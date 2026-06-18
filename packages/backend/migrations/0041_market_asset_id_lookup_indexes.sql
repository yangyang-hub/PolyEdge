-- Speed up orderbook priority sync token -> condition lookups.
CREATE INDEX IF NOT EXISTS idx_markets_polymarket_yes_asset_id
    ON markets (polymarket_yes_asset_id)
    WHERE polymarket_yes_asset_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_markets_polymarket_no_asset_id
    ON markets (polymarket_no_asset_id)
    WHERE polymarket_no_asset_id IS NOT NULL;
