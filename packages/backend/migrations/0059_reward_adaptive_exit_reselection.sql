ALTER TABLE reward_managed_orders
    ADD COLUMN exit_strategy_source TEXT NOT NULL DEFAULT 'configured'
        CHECK (exit_strategy_source IN ('configured', 'adaptive', 'external_inventory')),
    ADD COLUMN exit_strategy_selected TEXT
        CHECK (
            exit_strategy_selected IS NULL
            OR exit_strategy_selected IN (
                'exit_at_markup',
                'hold_and_requote',
                'flatten_immediately'
            )
        ),
    ADD COLUMN exit_floor_price NUMERIC(12, 6)
        CHECK (exit_floor_price IS NULL OR (exit_floor_price > 0 AND exit_floor_price < 1)),
    ADD COLUMN exit_reselect_count INTEGER NOT NULL DEFAULT 0
        CHECK (exit_reselect_count >= 0),
    ADD COLUMN exit_last_reselected_at TIMESTAMPTZ;
