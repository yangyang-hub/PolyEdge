CREATE TABLE reward_market_candles (
    token_id TEXT NOT NULL,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    outcome TEXT NOT NULL,
    interval_sec INTEGER NOT NULL CHECK (interval_sec > 0),
    bucket_start TIMESTAMPTZ NOT NULL,
    open NUMERIC(20,8) NOT NULL CHECK (open >= 0 AND open <= 1),
    high NUMERIC(20,8) NOT NULL CHECK (high >= 0 AND high <= 1),
    low NUMERIC(20,8) NOT NULL CHECK (low >= 0 AND low <= 1),
    close NUMERIC(20,8) NOT NULL CHECK (close >= 0 AND close <= 1),
    best_bid_close NUMERIC(20,8) NOT NULL CHECK (best_bid_close >= 0 AND best_bid_close <= 1),
    best_ask_close NUMERIC(20,8) NOT NULL CHECK (best_ask_close >= 0 AND best_ask_close <= 1),
    spread_cents_close NUMERIC(20,8) NOT NULL DEFAULT 0 CHECK (spread_cents_close >= 0),
    sample_count INTEGER NOT NULL DEFAULT 1 CHECK (sample_count > 0),
    close_observed_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (token_id, interval_sec, bucket_start)
);

CREATE INDEX reward_market_candles_condition_recent_idx
    ON reward_market_candles (condition_id, interval_sec, bucket_start DESC);

CREATE INDEX reward_market_candles_token_recent_idx
    ON reward_market_candles (token_id, interval_sec, bucket_start DESC);
