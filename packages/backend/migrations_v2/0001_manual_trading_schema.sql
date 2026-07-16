-- PolyEdge V3 clean-deploy PostgreSQL schema.
-- Manual market strategies, deterministic quote slots, and multi-wallet execution.
-- This schema intentionally contains no compatibility objects for earlier deployments.

CREATE TABLE wallet_credential_refs (
    credential_ref_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    provider TEXT NOT NULL CHECK (provider IN ('environment', 'vault', 'kms')),
    locator TEXT NOT NULL CHECK (length(btrim(locator)) > 0),
    key_version TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider, locator),
    CHECK (updated_at >= created_at)
);

CREATE TABLE wallet_accounts (
    wallet_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 120),
    signer_address TEXT NOT NULL CHECK (length(btrim(signer_address)) > 0),
    funder_address TEXT NOT NULL CHECK (length(btrim(funder_address)) > 0),
    signature_type INTEGER NOT NULL CHECK (signature_type BETWEEN 0 AND 2),
    credential_ref_id BIGINT NOT NULL UNIQUE
        REFERENCES wallet_credential_refs (credential_ref_id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'paused'
        CHECK (status IN ('active', 'paused', 'disabled', 'error')),
    trading_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (updated_at >= created_at),
    CHECK (status = 'active' OR trading_enabled = FALSE)
);

CREATE UNIQUE INDEX wallet_accounts_signer_address_uidx
    ON wallet_accounts (lower(signer_address));
CREATE UNIQUE INDEX wallet_accounts_name_uidx
    ON wallet_accounts (lower(name));
CREATE INDEX wallet_accounts_status_idx
    ON wallet_accounts (status, wallet_id);

CREATE TABLE wallet_risk_policies (
    wallet_id BIGINT PRIMARY KEY
        REFERENCES wallet_accounts (wallet_id) ON DELETE CASCADE,
    max_open_orders BIGINT NOT NULL CHECK (max_open_orders > 0),
    max_open_buy_notional NUMERIC(24, 8) NOT NULL
        CHECK (max_open_buy_notional >= 0),
    max_total_position_notional NUMERIC(24, 8) NOT NULL
        CHECK (max_total_position_notional >= 0),
    max_market_position_notional NUMERIC(24, 8) NOT NULL
        CHECK (max_market_position_notional >= 0),
    max_order_notional NUMERIC(24, 8) NOT NULL
        CHECK (max_order_notional > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (max_market_position_notional <= max_total_position_notional),
    CHECK (max_order_notional <= max_open_buy_notional)
);

CREATE TABLE wallet_account_state (
    wallet_id BIGINT PRIMARY KEY
        REFERENCES wallet_accounts (wallet_id) ON DELETE CASCADE,
    available_collateral NUMERIC(24, 8) NOT NULL DEFAULT 0
        CHECK (available_collateral >= 0),
    reserved_collateral NUMERIC(24, 8) NOT NULL DEFAULT 0
        CHECK (reserved_collateral >= 0),
    open_buy_notional NUMERIC(24, 8) NOT NULL DEFAULT 0
        CHECK (open_buy_notional >= 0),
    total_position_notional NUMERIC(24, 8) NOT NULL DEFAULT 0
        CHECK (total_position_notional >= 0),
    last_synced_at TIMESTAMPTZ,
    last_error TEXT,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
) WITH (fillfactor = 90);

CREATE INDEX wallet_account_state_synced_idx
    ON wallet_account_state (last_synced_at);

CREATE TABLE managed_markets (
    market_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    condition_id TEXT NOT NULL UNIQUE CHECK (length(btrim(condition_id)) > 0),
    slug TEXT NOT NULL CHECK (length(btrim(slug)) > 0),
    question TEXT NOT NULL CHECK (length(btrim(question)) > 0),
    polymarket_url TEXT,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'closed', 'resolved')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (updated_at >= created_at)
);

CREATE UNIQUE INDEX managed_markets_slug_uidx
    ON managed_markets (lower(slug));
CREATE INDEX managed_markets_status_idx
    ON managed_markets (status, updated_at DESC);

CREATE TABLE managed_market_outcomes (
    outcome_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    market_id BIGINT NOT NULL
        REFERENCES managed_markets (market_id) ON DELETE CASCADE,
    outcome TEXT NOT NULL CHECK (outcome IN ('yes', 'no')),
    token_id TEXT NOT NULL UNIQUE CHECK (length(btrim(token_id)) > 0),
    UNIQUE (market_id, outcome)
);

CREATE INDEX managed_market_outcomes_market_idx
    ON managed_market_outcomes (market_id, outcome);

CREATE TABLE market_reward_terms (
    market_id BIGINT PRIMARY KEY
        REFERENCES managed_markets (market_id) ON DELETE CASCADE,
    minimum_size NUMERIC(24, 8) NOT NULL CHECK (minimum_size > 0),
    maximum_spread NUMERIC(12, 10) NOT NULL
        CHECK (maximum_spread >= 0 AND maximum_spread <= 1),
    daily_rate NUMERIC(24, 8) CHECK (daily_rate IS NULL OR daily_rate >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE market_strategies (
    strategy_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    market_id BIGINT NOT NULL
        REFERENCES managed_markets (market_id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 160),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'active', 'paused', 'archived')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (market_id, name),
    CHECK (updated_at >= created_at)
);

CREATE INDEX market_strategies_market_idx
    ON market_strategies (market_id, status);
CREATE INDEX market_strategies_status_idx
    ON market_strategies (status, updated_at DESC);

CREATE TABLE strategy_versions (
    strategy_version_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    strategy_id BIGINT NOT NULL
        REFERENCES market_strategies (strategy_id) ON DELETE CASCADE,
    version_number BIGINT NOT NULL CHECK (version_number > 0),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'published', 'retired')),
    book_freshness_ms BIGINT NOT NULL CHECK (book_freshness_ms > 0),
    downward_reprice_confirm_ms BIGINT NOT NULL
        CHECK (downward_reprice_confirm_ms >= 0),
    upward_reprice_confirm_ms BIGINT NOT NULL
        CHECK (upward_reprice_confirm_ms >= 0),
    reprice_cooldown_ms BIGINT NOT NULL CHECK (reprice_cooldown_ms >= 0),
    max_replaces_per_cycle BIGINT NOT NULL
        CHECK (max_replaces_per_cycle >= 0),
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (strategy_id, version_number),
    CHECK (
        (status = 'published' AND published_at IS NOT NULL)
        OR (status <> 'published')
    )
);

CREATE UNIQUE INDEX strategy_versions_one_published_uidx
    ON strategy_versions (strategy_id)
    WHERE status = 'published';
CREATE INDEX strategy_versions_strategy_idx
    ON strategy_versions (strategy_id, version_number DESC);

CREATE TABLE strategy_quote_slots (
    quote_slot_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    strategy_version_id BIGINT NOT NULL
        REFERENCES strategy_versions (strategy_version_id) ON DELETE CASCADE,
    slot_key TEXT NOT NULL CHECK (length(btrim(slot_key)) BETWEEN 1 AND 120),
    outcome TEXT NOT NULL CHECK (outcome IN ('yes', 'no')),
    quantity NUMERIC(24, 8) NOT NULL CHECK (quantity > 0),
    pricing_mode TEXT NOT NULL CHECK (pricing_mode IN ('fixed', 'book_rank')),
    fixed_price NUMERIC(12, 10),
    book_rank BIGINT,
    price_offset NUMERIC(12, 10) NOT NULL DEFAULT 0,
    minimum_price NUMERIC(12, 10) NOT NULL,
    maximum_price NUMERIC(12, 10) NOT NULL,
    post_only BOOLEAN NOT NULL DEFAULT TRUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (strategy_version_id, slot_key),
    CHECK (minimum_price > 0 AND minimum_price < 1),
    CHECK (maximum_price > 0 AND maximum_price < 1),
    CHECK (minimum_price <= maximum_price),
    CHECK (
        (pricing_mode = 'fixed' AND fixed_price IS NOT NULL AND book_rank IS NULL)
        OR
        (pricing_mode = 'book_rank' AND fixed_price IS NULL AND book_rank > 0)
    ),
    CHECK (fixed_price IS NULL OR (fixed_price >= minimum_price AND fixed_price <= maximum_price))
);

CREATE INDEX strategy_quote_slots_version_idx
    ON strategy_quote_slots (strategy_version_id, enabled, outcome);

CREATE TABLE strategy_wallet_targets (
    strategy_id BIGINT NOT NULL
        REFERENCES market_strategies (strategy_id) ON DELETE CASCADE,
    wallet_id BIGINT NOT NULL
        REFERENCES wallet_accounts (wallet_id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (strategy_id, wallet_id)
);

CREATE INDEX strategy_wallet_targets_wallet_idx
    ON strategy_wallet_targets (wallet_id, enabled, strategy_id);

CREATE TABLE execution_batches (
    batch_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    strategy_version_id BIGINT NOT NULL
        REFERENCES strategy_versions (strategy_version_id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (
            status IN (
                'pending', 'running', 'partially_succeeded',
                'succeeded', 'failed', 'cancelled'
            )
        ),
    requested_by TEXT NOT NULL CHECK (length(btrim(requested_by)) > 0),
    operator_note TEXT CHECK (operator_note IS NULL OR length(operator_note) <= 500),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    CHECK (started_at IS NULL OR started_at >= created_at),
    CHECK (completed_at IS NULL OR completed_at >= created_at)
);

CREATE INDEX execution_batches_strategy_idx
    ON execution_batches (strategy_version_id, created_at DESC);
CREATE INDEX execution_batches_status_idx
    ON execution_batches (status, created_at, batch_id);

CREATE TABLE wallet_execution_jobs (
    job_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    batch_id BIGINT NOT NULL
        REFERENCES execution_batches (batch_id) ON DELETE CASCADE,
    wallet_id BIGINT NOT NULL
        REFERENCES wallet_accounts (wallet_id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'succeeded', 'failed', 'cancelled')),
    attempt_count BIGINT NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    error_code TEXT,
    error_message TEXT,
    lease_owner TEXT,
    lease_epoch BIGINT NOT NULL DEFAULT 0 CHECK (lease_epoch >= 0),
    lease_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    UNIQUE (batch_id, wallet_id),
    CHECK ((lease_owner IS NULL) = (lease_expires_at IS NULL)),
    CHECK (
        status <> 'running'
        OR (lease_owner IS NOT NULL AND lease_epoch > 0 AND lease_expires_at IS NOT NULL)
    ),
    CHECK (updated_at >= created_at),
    CHECK (started_at IS NULL OR started_at >= created_at),
    CHECK (completed_at IS NULL OR completed_at >= created_at)
) WITH (fillfactor = 90);

CREATE INDEX wallet_execution_jobs_wallet_idx
    ON wallet_execution_jobs (wallet_id, created_at DESC);
CREATE INDEX wallet_execution_jobs_claim_idx
    ON wallet_execution_jobs (status, lease_expires_at, created_at, job_id)
    WHERE status IN ('pending', 'running');

CREATE TABLE managed_orders (
    managed_order_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    wallet_id BIGINT NOT NULL
        REFERENCES wallet_accounts (wallet_id) ON DELETE RESTRICT,
    market_id BIGINT NOT NULL
        REFERENCES managed_markets (market_id) ON DELETE RESTRICT,
    strategy_version_id BIGINT NOT NULL
        REFERENCES strategy_versions (strategy_version_id) ON DELETE RESTRICT,
    quote_slot_id BIGINT
        REFERENCES strategy_quote_slots (quote_slot_id) ON DELETE RESTRICT,
    token_id TEXT NOT NULL CHECK (length(btrim(token_id)) > 0),
    outcome TEXT NOT NULL CHECK (outcome IN ('yes', 'no')),
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 10) NOT NULL CHECK (price > 0 AND price < 1),
    quantity NUMERIC(24, 8) NOT NULL CHECK (quantity > 0),
    filled_quantity NUMERIC(24, 8) NOT NULL DEFAULT 0
        CHECK (filled_quantity >= 0 AND filled_quantity <= quantity),
    status TEXT NOT NULL
        CHECK (
            status IN (
                'planned', 'submitting', 'open', 'partially_filled',
                'cancel_pending', 'cancelled', 'filled', 'expired',
                'rejected', 'unknown'
            )
        ),
    external_order_id TEXT,
    client_order_key TEXT NOT NULL UNIQUE,
    generation BIGINT NOT NULL CHECK (generation > 0),
    last_venue_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (updated_at >= created_at),
    CHECK (
        external_order_id IS NOT NULL
        OR status IN ('planned', 'submitting', 'rejected', 'unknown')
    )
) WITH (fillfactor = 90);

CREATE UNIQUE INDEX managed_orders_external_order_uidx
    ON managed_orders (wallet_id, external_order_id)
    WHERE external_order_id IS NOT NULL;
CREATE UNIQUE INDEX managed_orders_open_slot_uidx
    ON managed_orders (wallet_id, quote_slot_id)
    WHERE quote_slot_id IS NOT NULL
      AND status IN (
          'planned', 'submitting', 'open', 'partially_filled',
          'cancel_pending', 'unknown'
      );
CREATE UNIQUE INDEX managed_orders_slot_generation_uidx
    ON managed_orders (wallet_id, quote_slot_id, generation)
    WHERE quote_slot_id IS NOT NULL;
CREATE INDEX managed_orders_wallet_status_idx
    ON managed_orders (wallet_id, status, updated_at DESC);
CREATE INDEX managed_orders_market_idx
    ON managed_orders (market_id, wallet_id, status);
CREATE INDEX managed_orders_strategy_version_idx
    ON managed_orders (strategy_version_id, status);
CREATE INDEX managed_orders_quote_slot_idx
    ON managed_orders (quote_slot_id, created_at DESC)
    WHERE quote_slot_id IS NOT NULL;

CREATE TABLE execution_actions (
    action_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    job_id BIGINT NOT NULL
        REFERENCES wallet_execution_jobs (job_id) ON DELETE CASCADE,
    quote_slot_id BIGINT
        REFERENCES strategy_quote_slots (quote_slot_id) ON DELETE RESTRICT,
    managed_order_id BIGINT
        REFERENCES managed_orders (managed_order_id) ON DELETE SET NULL,
    action_type TEXT NOT NULL
        CHECK (
            action_type IN (
                'place_order', 'cancel_order', 'replace_order', 'reconcile_order'
            )
        ),
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (
            status IN (
                'planned', 'executing', 'succeeded',
                'failed', 'unknown', 'cancelled'
            )
        ),
    idempotency_key TEXT NOT NULL UNIQUE,
    reason_code TEXT NOT NULL CHECK (length(btrim(reason_code)) > 0),
    request_json JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(request_json) = 'object'),
    result_json JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(result_json) = 'object'),
    attempt_count BIGINT NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    lease_owner TEXT,
    lease_epoch BIGINT NOT NULL DEFAULT 0 CHECK (lease_epoch >= 0),
    lease_expires_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    CHECK ((lease_owner IS NULL) = (lease_expires_at IS NULL)),
    CHECK (
        status <> 'executing'
        OR (lease_owner IS NOT NULL AND lease_epoch > 0 AND lease_expires_at IS NOT NULL)
    ),
    CHECK (updated_at >= created_at),
    CHECK (completed_at IS NULL OR completed_at >= created_at)
) WITH (fillfactor = 90);

CREATE INDEX execution_actions_job_idx
    ON execution_actions (job_id, created_at, action_id);
CREATE INDEX execution_actions_quote_slot_idx
    ON execution_actions (quote_slot_id, created_at DESC)
    WHERE quote_slot_id IS NOT NULL;
CREATE INDEX execution_actions_managed_order_idx
    ON execution_actions (managed_order_id, created_at DESC)
    WHERE managed_order_id IS NOT NULL;
CREATE INDEX execution_actions_claim_idx
    ON execution_actions (status, lease_expires_at, created_at, action_id)
    WHERE status IN ('planned', 'executing');

CREATE TABLE order_transitions (
    transition_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    managed_order_id BIGINT NOT NULL
        REFERENCES managed_orders (managed_order_id) ON DELETE CASCADE,
    action_id BIGINT
        REFERENCES execution_actions (action_id) ON DELETE SET NULL,
    from_status TEXT,
    to_status TEXT NOT NULL
        CHECK (
            to_status IN (
                'planned', 'submitting', 'open', 'partially_filled',
                'cancel_pending', 'cancelled', 'filled', 'expired',
                'rejected', 'unknown'
            )
        ),
    reason_code TEXT NOT NULL CHECK (length(btrim(reason_code)) > 0),
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(metadata_json) = 'object'),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        from_status IS NULL
        OR from_status IN (
            'planned', 'submitting', 'open', 'partially_filled',
            'cancel_pending', 'cancelled', 'filled', 'expired',
            'rejected', 'unknown'
        )
    )
);

CREATE INDEX order_transitions_order_idx
    ON order_transitions (managed_order_id, occurred_at DESC, transition_id DESC);
CREATE INDEX order_transitions_action_idx
    ON order_transitions (action_id, occurred_at DESC)
    WHERE action_id IS NOT NULL;

CREATE TABLE positions (
    position_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    wallet_id BIGINT NOT NULL
        REFERENCES wallet_accounts (wallet_id) ON DELETE CASCADE,
    market_id BIGINT NOT NULL
        REFERENCES managed_markets (market_id) ON DELETE RESTRICT,
    token_id TEXT NOT NULL CHECK (length(btrim(token_id)) > 0),
    outcome TEXT NOT NULL CHECK (outcome IN ('yes', 'no')),
    quantity NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (quantity >= 0),
    average_price NUMERIC(12, 10) NOT NULL DEFAULT 0
        CHECK (average_price >= 0 AND average_price < 1),
    realized_pnl NUMERIC(24, 8) NOT NULL DEFAULT 0,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    observed_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_id, token_id)
) WITH (fillfactor = 90);

CREATE INDEX positions_market_idx
    ON positions (market_id, wallet_id);
CREATE INDEX positions_wallet_nonzero_idx
    ON positions (wallet_id, updated_at DESC)
    WHERE quantity > 0;

CREATE TABLE idempotency_keys (
    idempotency_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    scope TEXT NOT NULL CHECK (length(btrim(scope)) > 0),
    idempotency_key TEXT NOT NULL CHECK (length(btrim(idempotency_key)) > 0),
    request_hash TEXT NOT NULL CHECK (length(btrim(request_hash)) > 0),
    owner_token TEXT NOT NULL CHECK (length(btrim(owner_token)) > 0),
    status TEXT NOT NULL CHECK (status IN ('started', 'completed', 'failed')),
    response_json JSONB,
    error_code TEXT,
    lease_epoch BIGINT NOT NULL DEFAULT 1 CHECK (lease_epoch > 0),
    lease_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL,
    UNIQUE (scope, idempotency_key),
    CHECK (updated_at >= created_at),
    CHECK (expires_at > created_at),
    CHECK (status <> 'started' OR lease_expires_at IS NOT NULL)
) WITH (fillfactor = 90);

CREATE INDEX idempotency_keys_claim_idx
    ON idempotency_keys (lease_expires_at, idempotency_id)
    WHERE status = 'started';
CREATE INDEX idempotency_keys_expiry_idx
    ON idempotency_keys (expires_at);

CREATE TABLE audit_logs (
    audit_log_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    request_id TEXT NOT NULL CHECK (length(btrim(request_id)) > 0),
    actor_id TEXT NOT NULL CHECK (length(btrim(actor_id)) > 0),
    action TEXT NOT NULL CHECK (length(btrim(action)) > 0),
    resource_type TEXT NOT NULL CHECK (length(btrim(resource_type)) > 0),
    resource_id TEXT NOT NULL CHECK (length(btrim(resource_id)) > 0),
    result TEXT NOT NULL
        CHECK (result IN ('accepted', 'succeeded', 'rejected', 'failed')),
    operator_note TEXT CHECK (operator_note IS NULL OR length(operator_note) <= 500),
    error_code TEXT,
    payload_json JSONB
        CHECK (payload_json IS NULL OR jsonb_typeof(payload_json) = 'object')
);

CREATE INDEX audit_logs_request_idx
    ON audit_logs (request_id);
CREATE INDEX audit_logs_actor_idx
    ON audit_logs (actor_id, occurred_at DESC);
CREATE INDEX audit_logs_resource_idx
    ON audit_logs (resource_type, resource_id, occurred_at DESC);
CREATE INDEX audit_logs_action_idx
    ON audit_logs (action, occurred_at DESC);

CREATE TABLE system_runtime_state (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    kill_switch_locked BOOLEAN NOT NULL DEFAULT TRUE,
    trading_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    reason TEXT,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    updated_by TEXT NOT NULL DEFAULT 'system',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (kill_switch_locked = FALSE OR trading_enabled = FALSE)
);

INSERT INTO system_runtime_state (
    singleton,
    kill_switch_locked,
    trading_enabled,
    reason,
    version,
    updated_by
) VALUES (
    TRUE,
    TRUE,
    FALSE,
    'clean deploy starts fail-closed',
    0,
    'schema'
);
