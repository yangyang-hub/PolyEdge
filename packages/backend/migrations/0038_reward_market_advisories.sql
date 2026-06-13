CREATE TABLE reward_market_advisories (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('openai', 'anthropic')),
    request_format TEXT NOT NULL CHECK (
        request_format IN (
            'openai_responses',
            'openai_chat_completions',
            'anthropic_messages'
        )
    ),
    model TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    suitability TEXT NOT NULL CHECK (suitability IN ('allow', 'avoid', 'watch')),
    quote_mode TEXT NOT NULL CHECK (quote_mode IN ('double', 'single_yes', 'single_no', 'none')),
    exit_policy TEXT NOT NULL,
    confidence NUMERIC(5, 4) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    reasons_json JSONB NOT NULL CHECK (jsonb_typeof(reasons_json) = 'array'),
    metrics_json JSONB NOT NULL CHECK (jsonb_typeof(metrics_json) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_reward_market_advisories_condition_expires
    ON reward_market_advisories (condition_id, expires_at DESC);

CREATE INDEX idx_reward_market_advisories_provider_input
    ON reward_market_advisories (provider, request_format, model, input_hash);
