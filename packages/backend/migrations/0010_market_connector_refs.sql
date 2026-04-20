ALTER TABLE markets
    ADD COLUMN polymarket_condition_id TEXT,
    ADD COLUMN polymarket_yes_asset_id TEXT,
    ADD COLUMN polymarket_no_asset_id TEXT;

CREATE UNIQUE INDEX markets_polymarket_condition_id_uidx
    ON markets (polymarket_condition_id)
    WHERE polymarket_condition_id IS NOT NULL;
