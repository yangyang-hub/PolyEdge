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
