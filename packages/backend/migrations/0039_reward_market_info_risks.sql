CREATE TABLE reward_market_info_risks (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
  provider TEXT NOT NULL CHECK (provider IN ('openai', 'anthropic')),
  request_format TEXT NOT NULL CHECK (
    request_format IN ('openai_responses', 'openai_chat_completions', 'anthropic_messages')
  ),
  model TEXT NOT NULL,
  query_hash TEXT NOT NULL,
  input_hash TEXT NOT NULL,
  risk_level TEXT NOT NULL CHECK (
    risk_level IN ('low', 'medium', 'high', 'critical', 'unknown')
  ),
  risk_type TEXT NOT NULL CHECK (
    risk_type IN (
      'imminent_resolution',
      'breaking_news',
      'scheduled_event',
      'official_result',
      'rumor',
      'stale',
      'none',
      'unknown'
    )
  ),
  directional_risk TEXT NOT NULL CHECK (directional_risk IN ('yes', 'no', 'unclear')),
  resolution_imminent BOOLEAN NOT NULL DEFAULT false,
  expected_event_at TIMESTAMPTZ,
  confidence NUMERIC(5,4) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
  summary TEXT NOT NULL,
  sources_json JSONB NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(sources_json) = 'array'),
  metrics_json JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metrics_json) = 'object'),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_reward_market_info_risks_condition_expires
  ON reward_market_info_risks (condition_id, expires_at DESC);

CREATE INDEX idx_reward_market_info_risks_level_expires
  ON reward_market_info_risks (risk_level, expires_at DESC);

CREATE INDEX idx_reward_market_info_risks_request_cache
  ON reward_market_info_risks (
    condition_id,
    provider,
    request_format,
    model,
    input_hash,
    expires_at DESC
  );
