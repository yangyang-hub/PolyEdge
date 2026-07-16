-- PolyEdge V4 clean-deploy PostgreSQL schema.
-- Multi-user manual market making, strategy following, and encrypted wallet custody.
-- This schema intentionally contains no compatibility objects for earlier deployments.

CREATE TABLE users (
    user_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    username TEXT NOT NULL UNIQUE CHECK (length(btrim(username)) BETWEEN 3 AND 64),
    display_name TEXT NOT NULL CHECK (length(btrim(display_name)) BETWEEN 1 AND 120),
    role TEXT NOT NULL CHECK (role IN ('admin', 'market_editor', 'read_only')),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'active', 'locked', 'disabled')),
    auth_source TEXT NOT NULL DEFAULT 'local'
        CHECK (auth_source IN ('local', 'environment_admin')),
    created_by_user_id BIGINT REFERENCES users (user_id) ON DELETE RESTRICT,
    credential_version BIGINT NOT NULL DEFAULT 1 CHECK (credential_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (updated_at >= created_at),
    CHECK (auth_source <> 'environment_admin' OR role = 'admin'),
    CHECK (auth_source = 'environment_admin' OR created_by_user_id IS NOT NULL)
);

CREATE UNIQUE INDEX users_username_uidx ON users (lower(username));
CREATE UNIQUE INDEX users_one_bootstrap_admin_uidx
    ON users (auth_source) WHERE auth_source = 'environment_admin';
CREATE INDEX users_created_by_idx
    ON users (created_by_user_id, created_at DESC)
    WHERE created_by_user_id IS NOT NULL;
CREATE INDEX users_status_role_idx ON users (status, role, user_id);

CREATE TABLE user_password_credentials (
    user_id BIGINT PRIMARY KEY REFERENCES users (user_id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL CHECK (length(btrim(password_hash)) > 0),
    credential_version BIGINT NOT NULL CHECK (credential_version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE user_sessions (
    session_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) >= 32),
    csrf_token_hash BYTEA NOT NULL CHECK (octet_length(csrf_token_hash) >= 32),
    recent_auth_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    absolute_expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (recent_auth_at >= created_at),
    CHECK (last_seen_at >= created_at),
    CHECK (expires_at > created_at),
    CHECK (absolute_expires_at >= expires_at),
    CHECK (revoked_at IS NULL OR revoked_at >= created_at)
) WITH (fillfactor = 90);

CREATE INDEX user_sessions_user_active_idx
    ON user_sessions (user_id, absolute_expires_at DESC)
    WHERE revoked_at IS NULL;
CREATE INDEX user_sessions_expiry_idx
    ON user_sessions (expires_at, absolute_expires_at)
    WHERE revoked_at IS NULL;

CREATE TABLE user_activation_tokens (
    activation_token_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) >= 32),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_by_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at > created_at),
    CHECK (used_at IS NULL OR used_at >= created_at)
);

CREATE UNIQUE INDEX user_activation_tokens_one_live_uidx
    ON user_activation_tokens (user_id) WHERE used_at IS NULL;
CREATE INDEX user_activation_tokens_expiry_idx
    ON user_activation_tokens (expires_at) WHERE used_at IS NULL;
CREATE INDEX user_activation_tokens_creator_idx
    ON user_activation_tokens (created_by_user_id, created_at DESC);

CREATE TABLE auth_login_attempts (
    login_attempt_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    username_key TEXT NOT NULL CHECK (length(btrim(username_key)) > 0),
    succeeded BOOLEAN NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX auth_login_attempts_username_time_idx
    ON auth_login_attempts (username_key, occurred_at DESC);



CREATE TABLE wallet_accounts (
    wallet_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 120),
    signer_address TEXT NOT NULL CHECK (length(btrim(signer_address)) > 0),
    funder_address TEXT NOT NULL CHECK (length(btrim(funder_address)) > 0),
    signature_type INTEGER NOT NULL CHECK (signature_type BETWEEN 0 AND 2),
    status TEXT NOT NULL DEFAULT 'paused'
        CHECK (status IN ('active', 'paused', 'disabled', 'error')),
    trading_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (owner_user_id, wallet_id),
    CHECK (updated_at >= created_at),
    CHECK (status = 'active' OR trading_enabled = FALSE)
);

CREATE UNIQUE INDEX wallet_accounts_signer_address_uidx
    ON wallet_accounts (lower(signer_address));
CREATE UNIQUE INDEX wallet_accounts_owner_name_uidx
    ON wallet_accounts (owner_user_id, lower(name));
CREATE INDEX wallet_accounts_owner_status_idx
    ON wallet_accounts (owner_user_id, status, wallet_id);

CREATE TABLE wallet_secret_envelopes (
    wallet_id BIGINT PRIMARY KEY,
    owner_user_id BIGINT NOT NULL,
    ciphertext BYTEA NOT NULL CHECK (octet_length(ciphertext) > 0),
    payload_nonce BYTEA NOT NULL CHECK (octet_length(payload_nonce) = 12),
    wrapped_dek BYTEA NOT NULL CHECK (octet_length(wrapped_dek) > 0),
    wrapped_dek_nonce BYTEA NOT NULL CHECK (octet_length(wrapped_dek_nonce) = 12),
    key_id TEXT NOT NULL CHECK (length(btrim(key_id)) > 0),
    algorithm TEXT NOT NULL DEFAULT 'aes-256-gcm+wrapped-dek'
        CHECK (algorithm = 'aes-256-gcm+wrapped-dek'),
    aad_version BIGINT NOT NULL DEFAULT 1 CHECK (aad_version > 0),
    secret_version BIGINT NOT NULL DEFAULT 1 CHECK (secret_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (owner_user_id, wallet_id)
        REFERENCES wallet_accounts (owner_user_id, wallet_id) ON DELETE CASCADE,
    CHECK (updated_at >= created_at)
);

CREATE INDEX wallet_secret_envelopes_owner_idx
    ON wallet_secret_envelopes (owner_user_id, wallet_id);
CREATE INDEX wallet_secret_envelopes_key_idx
    ON wallet_secret_envelopes (key_id, secret_version);

CREATE TABLE wallet_import_contexts (
    import_context_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    transport_key_id TEXT NOT NULL CHECK (length(btrim(transport_key_id)) > 0),
    context_token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(context_token_hash) >= 32),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at > created_at),
    CHECK (consumed_at IS NULL OR consumed_at >= created_at)
);

CREATE INDEX wallet_import_contexts_owner_idx
    ON wallet_import_contexts (owner_user_id, created_at DESC);
CREATE INDEX wallet_import_contexts_expiry_idx
    ON wallet_import_contexts (expires_at) WHERE consumed_at IS NULL;

CREATE TABLE wallet_risk_policies (
    wallet_id BIGINT PRIMARY KEY REFERENCES wallet_accounts (wallet_id) ON DELETE CASCADE,
    max_open_orders BIGINT NOT NULL CHECK (max_open_orders > 0),
    max_open_buy_notional NUMERIC(24, 8) NOT NULL CHECK (max_open_buy_notional >= 0),
    max_total_position_notional NUMERIC(24, 8) NOT NULL CHECK (max_total_position_notional >= 0),
    max_market_position_notional NUMERIC(24, 8) NOT NULL CHECK (max_market_position_notional >= 0),
    max_order_notional NUMERIC(24, 8) NOT NULL CHECK (max_order_notional > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (max_market_position_notional <= max_total_position_notional),
    CHECK (max_order_notional <= max_open_buy_notional)
);

CREATE TABLE wallet_account_state (
    wallet_id BIGINT PRIMARY KEY REFERENCES wallet_accounts (wallet_id) ON DELETE CASCADE,
    available_collateral NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (available_collateral >= 0),
    reserved_collateral NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (reserved_collateral >= 0),
    open_buy_notional NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (open_buy_notional >= 0),
    total_position_notional NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (total_position_notional >= 0),
    last_synced_at TIMESTAMPTZ,
    last_error TEXT,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
) WITH (fillfactor = 90);

CREATE INDEX wallet_account_state_synced_idx ON wallet_account_state (last_synced_at);

CREATE TABLE managed_markets (
    market_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_by_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    condition_id TEXT NOT NULL UNIQUE CHECK (length(btrim(condition_id)) > 0),
    slug TEXT NOT NULL CHECK (length(btrim(slug)) > 0),
    question TEXT NOT NULL CHECK (length(btrim(question)) > 0),
    polymarket_url TEXT,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'resolved')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (updated_at >= created_at)
);

CREATE UNIQUE INDEX managed_markets_slug_uidx ON managed_markets (lower(slug));
CREATE INDEX managed_markets_status_idx ON managed_markets (status, updated_at DESC);
CREATE INDEX managed_markets_creator_idx
    ON managed_markets (created_by_user_id, created_at DESC);

CREATE TABLE managed_market_outcomes (
    outcome_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    market_id BIGINT NOT NULL REFERENCES managed_markets (market_id) ON DELETE CASCADE,
    outcome TEXT NOT NULL CHECK (outcome IN ('yes', 'no')),
    token_id TEXT NOT NULL UNIQUE CHECK (length(btrim(token_id)) > 0),
    UNIQUE (market_id, outcome)
);

CREATE INDEX managed_market_outcomes_market_idx
    ON managed_market_outcomes (market_id, outcome);

CREATE TABLE market_strategies (
    strategy_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    market_id BIGINT NOT NULL REFERENCES managed_markets (market_id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 160),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'active', 'paused', 'expired', 'archived')),
    visibility TEXT NOT NULL DEFAULT 'private'
        CHECK (visibility IN ('private', 'followable')),
    active_from TIMESTAMPTZ NOT NULL DEFAULT now(),
    active_until TIMESTAMPTZ NOT NULL,
    expired_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (owner_user_id, market_id, name),
    UNIQUE (owner_user_id, strategy_id),
    CHECK (active_until > active_from),
    CHECK (expired_at IS NULL OR expired_at >= active_from),
    CHECK (updated_at >= created_at)
);

CREATE INDEX market_strategies_owner_idx
    ON market_strategies (owner_user_id, status, updated_at DESC);
CREATE INDEX market_strategies_market_idx
    ON market_strategies (market_id, status, active_until);
CREATE INDEX market_strategies_expiry_idx
    ON market_strategies (active_until, strategy_id)
    WHERE status IN ('active', 'paused');
CREATE INDEX market_strategies_followable_idx
    ON market_strategies (updated_at DESC, strategy_id)
    WHERE visibility = 'followable' AND status <> 'archived';

CREATE TABLE strategy_versions (
    strategy_version_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    strategy_id BIGINT NOT NULL REFERENCES market_strategies (strategy_id) ON DELETE CASCADE,
    version_number BIGINT NOT NULL CHECK (version_number > 0),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'published', 'retired')),
    reward_minimum_size NUMERIC(24, 8) NOT NULL CHECK (reward_minimum_size > 0),
    reward_maximum_spread NUMERIC(12, 10) NOT NULL
        CHECK (reward_maximum_spread >= 0 AND reward_maximum_spread <= 1),
    reward_daily_rate NUMERIC(24, 8)
        CHECK (reward_daily_rate IS NULL OR reward_daily_rate >= 0),
    book_freshness_ms BIGINT NOT NULL CHECK (book_freshness_ms > 0),
    downward_reprice_confirm_ms BIGINT NOT NULL CHECK (downward_reprice_confirm_ms >= 0),
    upward_reprice_confirm_ms BIGINT NOT NULL CHECK (upward_reprice_confirm_ms >= 0),
    reprice_cooldown_ms BIGINT NOT NULL CHECK (reprice_cooldown_ms >= 0),
    max_replaces_per_cycle BIGINT NOT NULL CHECK (max_replaces_per_cycle >= 0),
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (strategy_id, version_number),
    UNIQUE (strategy_id, strategy_version_id),
    CHECK (
        (status = 'published' AND published_at IS NOT NULL)
        OR (status <> 'published')
    )
);

CREATE UNIQUE INDEX strategy_versions_one_published_uidx
    ON strategy_versions (strategy_id) WHERE status = 'published';
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

CREATE TABLE strategy_subscriptions (
    subscription_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    follower_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    source_strategy_id BIGINT NOT NULL REFERENCES market_strategies (strategy_id) ON DELETE RESTRICT,
    subscription_kind TEXT NOT NULL DEFAULT 'follower'
        CHECK (subscription_kind IN ('owner', 'follower')),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'paused', 'stopped', 'expired')),
    active_until TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (follower_user_id, source_strategy_id),
    UNIQUE (subscription_id, follower_user_id),
    CHECK (active_until IS NULL OR active_until > created_at),
    CHECK (stopped_at IS NULL OR stopped_at >= created_at),
    CHECK (updated_at >= created_at)
);

CREATE INDEX strategy_subscriptions_source_idx
    ON strategy_subscriptions (source_strategy_id, status, subscription_id);
CREATE INDEX strategy_subscriptions_follower_idx
    ON strategy_subscriptions (follower_user_id, status, updated_at DESC);
CREATE INDEX strategy_subscriptions_expiry_idx
    ON strategy_subscriptions (active_until, subscription_id)
    WHERE status IN ('active', 'paused') AND active_until IS NOT NULL;

CREATE TABLE strategy_subscription_wallets (
    subscription_id BIGINT NOT NULL,
    follower_user_id BIGINT NOT NULL,
    wallet_id BIGINT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (subscription_id, wallet_id),
    FOREIGN KEY (subscription_id, follower_user_id)
        REFERENCES strategy_subscriptions (subscription_id, follower_user_id) ON DELETE CASCADE,
    FOREIGN KEY (follower_user_id, wallet_id)
        REFERENCES wallet_accounts (owner_user_id, wallet_id) ON DELETE CASCADE
);

CREATE INDEX strategy_subscription_wallets_wallet_idx
    ON strategy_subscription_wallets (follower_user_id, wallet_id, enabled);

CREATE TABLE strategy_commands (
    command_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    source_strategy_id BIGINT NOT NULL REFERENCES market_strategies (strategy_id) ON DELETE CASCADE,
    source_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    strategy_version_id BIGINT,
    command_sequence BIGINT NOT NULL CHECK (command_sequence > 0),
    command_type TEXT NOT NULL
        CHECK (command_type IN ('publish', 'activate', 'pause', 'resume', 'expire', 'archive', 'force_cancel')),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    payload_json JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(payload_json) = 'object'),
    lease_owner TEXT,
    lease_epoch BIGINT NOT NULL DEFAULT 0 CHECK (lease_epoch >= 0),
    lease_expires_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    UNIQUE (source_strategy_id, command_sequence),
    FOREIGN KEY (source_user_id, source_strategy_id)
        REFERENCES market_strategies (owner_user_id, strategy_id) ON DELETE RESTRICT,
    FOREIGN KEY (source_strategy_id, strategy_version_id)
        REFERENCES strategy_versions (strategy_id, strategy_version_id) ON DELETE RESTRICT,
    CHECK ((lease_owner IS NULL) = (lease_expires_at IS NULL)),
    CHECK (
        status <> 'running'
        OR (lease_owner IS NOT NULL AND lease_epoch > 0 AND lease_expires_at IS NOT NULL)
    ),
    CHECK (updated_at >= created_at),
    CHECK (completed_at IS NULL OR completed_at >= created_at)
) WITH (fillfactor = 90);

CREATE INDEX strategy_commands_claim_idx
    ON strategy_commands (status, lease_expires_at, created_at, command_id)
    WHERE status IN ('pending', 'running');
CREATE INDEX strategy_commands_strategy_idx
    ON strategy_commands (source_strategy_id, command_sequence DESC);

CREATE TABLE execution_batches (
    batch_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    subscriber_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    subscription_id BIGINT NOT NULL,
    source_strategy_id BIGINT NOT NULL REFERENCES market_strategies (strategy_id) ON DELETE RESTRICT,
    strategy_version_id BIGINT NOT NULL,
    strategy_command_id BIGINT REFERENCES strategy_commands (command_id) ON DELETE SET NULL,
    batch_type TEXT NOT NULL DEFAULT 'execute' CHECK (batch_type IN ('execute', 'cancel')),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'partially_succeeded', 'succeeded', 'failed', 'cancelled')),
    requested_by_user_id BIGINT REFERENCES users (user_id) ON DELETE RESTRICT,
    request_source TEXT NOT NULL DEFAULT 'operator'
        CHECK (request_source IN ('operator', 'runtime', 'strategy_command', 'expiry_supervisor')),
    operator_note TEXT CHECK (operator_note IS NULL OR length(operator_note) <= 500),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    FOREIGN KEY (subscription_id, subscriber_user_id)
        REFERENCES strategy_subscriptions (subscription_id, follower_user_id) ON DELETE RESTRICT,
    FOREIGN KEY (source_strategy_id, strategy_version_id)
        REFERENCES strategy_versions (strategy_id, strategy_version_id) ON DELETE RESTRICT,
    CHECK (request_source <> 'operator' OR requested_by_user_id IS NOT NULL),
    CHECK (started_at IS NULL OR started_at >= created_at),
    CHECK (completed_at IS NULL OR completed_at >= created_at)
);

CREATE INDEX execution_batches_user_idx
    ON execution_batches (subscriber_user_id, created_at DESC);
CREATE INDEX execution_batches_strategy_idx
    ON execution_batches (source_strategy_id, strategy_version_id, created_at DESC);
CREATE INDEX execution_batches_status_idx
    ON execution_batches (status, created_at, batch_id);
CREATE INDEX execution_batches_command_idx
    ON execution_batches (strategy_command_id, subscriber_user_id)
    WHERE strategy_command_id IS NOT NULL;

CREATE TABLE wallet_execution_jobs (
    job_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    batch_id BIGINT NOT NULL REFERENCES execution_batches (batch_id) ON DELETE CASCADE,
    owner_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    wallet_id BIGINT NOT NULL,
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
    FOREIGN KEY (owner_user_id, wallet_id)
        REFERENCES wallet_accounts (owner_user_id, wallet_id) ON DELETE RESTRICT,
    CHECK ((lease_owner IS NULL) = (lease_expires_at IS NULL)),
    CHECK (
        status <> 'running'
        OR (lease_owner IS NOT NULL AND lease_epoch > 0 AND lease_expires_at IS NOT NULL)
    ),
    CHECK (updated_at >= created_at),
    CHECK (started_at IS NULL OR started_at >= created_at),
    CHECK (completed_at IS NULL OR completed_at >= created_at)
) WITH (fillfactor = 90);

CREATE INDEX wallet_execution_jobs_user_idx
    ON wallet_execution_jobs (owner_user_id, created_at DESC);
CREATE INDEX wallet_execution_jobs_wallet_idx
    ON wallet_execution_jobs (wallet_id, created_at DESC);
CREATE INDEX wallet_execution_jobs_claim_idx
    ON wallet_execution_jobs (status, lease_expires_at, created_at, job_id)
    WHERE status IN ('pending', 'running');

CREATE TABLE managed_orders (
    managed_order_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    wallet_id BIGINT NOT NULL,
    subscription_id BIGINT NOT NULL,
    market_id BIGINT NOT NULL REFERENCES managed_markets (market_id) ON DELETE RESTRICT,
    strategy_version_id BIGINT NOT NULL REFERENCES strategy_versions (strategy_version_id) ON DELETE RESTRICT,
    quote_slot_id BIGINT REFERENCES strategy_quote_slots (quote_slot_id) ON DELETE RESTRICT,
    token_id TEXT NOT NULL CHECK (length(btrim(token_id)) > 0),
    outcome TEXT NOT NULL CHECK (outcome IN ('yes', 'no')),
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 10) NOT NULL CHECK (price > 0 AND price < 1),
    quantity NUMERIC(24, 8) NOT NULL CHECK (quantity > 0),
    filled_quantity NUMERIC(24, 8) NOT NULL DEFAULT 0
        CHECK (filled_quantity >= 0 AND filled_quantity <= quantity),
    status TEXT NOT NULL
        CHECK (status IN ('planned', 'submitting', 'open', 'partially_filled', 'cancel_pending', 'cancelled', 'filled', 'expired', 'rejected', 'unknown')),
    external_order_id TEXT,
    client_order_key TEXT NOT NULL UNIQUE,
    generation BIGINT NOT NULL CHECK (generation > 0),
    last_venue_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (owner_user_id, wallet_id)
        REFERENCES wallet_accounts (owner_user_id, wallet_id) ON DELETE RESTRICT,
    FOREIGN KEY (subscription_id, owner_user_id)
        REFERENCES strategy_subscriptions (subscription_id, follower_user_id) ON DELETE RESTRICT,
    CHECK (updated_at >= created_at),
    CHECK (external_order_id IS NOT NULL OR status IN ('planned', 'submitting', 'rejected', 'unknown'))
) WITH (fillfactor = 90);

CREATE UNIQUE INDEX managed_orders_external_order_uidx
    ON managed_orders (wallet_id, external_order_id) WHERE external_order_id IS NOT NULL;
CREATE UNIQUE INDEX managed_orders_open_slot_uidx
    ON managed_orders (wallet_id, quote_slot_id)
    WHERE quote_slot_id IS NOT NULL
      AND status IN ('planned', 'submitting', 'open', 'partially_filled', 'cancel_pending', 'unknown');
CREATE UNIQUE INDEX managed_orders_slot_generation_uidx
    ON managed_orders (wallet_id, quote_slot_id, generation) WHERE quote_slot_id IS NOT NULL;
CREATE INDEX managed_orders_user_status_idx
    ON managed_orders (owner_user_id, status, updated_at DESC);
CREATE INDEX managed_orders_wallet_status_idx
    ON managed_orders (wallet_id, status, updated_at DESC);
CREATE INDEX managed_orders_market_idx
    ON managed_orders (market_id, wallet_id, status);
CREATE INDEX managed_orders_strategy_version_idx
    ON managed_orders (strategy_version_id, status);
CREATE INDEX managed_orders_subscription_idx
    ON managed_orders (subscription_id, status, updated_at DESC);
CREATE INDEX managed_orders_quote_slot_idx
    ON managed_orders (quote_slot_id, created_at DESC) WHERE quote_slot_id IS NOT NULL;

CREATE TABLE execution_actions (
    action_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    job_id BIGINT NOT NULL REFERENCES wallet_execution_jobs (job_id) ON DELETE CASCADE,
    quote_slot_id BIGINT REFERENCES strategy_quote_slots (quote_slot_id) ON DELETE RESTRICT,
    managed_order_id BIGINT REFERENCES managed_orders (managed_order_id) ON DELETE SET NULL,
    action_type TEXT NOT NULL
        CHECK (action_type IN ('place_order', 'cancel_order', 'replace_order', 'reconcile_order')),
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'executing', 'succeeded', 'failed', 'unknown', 'cancelled')),
    idempotency_key TEXT NOT NULL UNIQUE,
    reason_code TEXT NOT NULL CHECK (length(btrim(reason_code)) > 0),
    request_json JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(request_json) = 'object'),
    result_json JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(result_json) = 'object'),
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

CREATE INDEX execution_actions_job_idx ON execution_actions (job_id, created_at, action_id);
CREATE INDEX execution_actions_quote_slot_idx
    ON execution_actions (quote_slot_id, created_at DESC) WHERE quote_slot_id IS NOT NULL;
CREATE INDEX execution_actions_managed_order_idx
    ON execution_actions (managed_order_id, created_at DESC) WHERE managed_order_id IS NOT NULL;
CREATE INDEX execution_actions_claim_idx
    ON execution_actions (status, lease_expires_at, created_at, action_id)
    WHERE status IN ('planned', 'executing');

CREATE TABLE order_transitions (
    transition_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    managed_order_id BIGINT NOT NULL REFERENCES managed_orders (managed_order_id) ON DELETE CASCADE,
    action_id BIGINT REFERENCES execution_actions (action_id) ON DELETE SET NULL,
    from_status TEXT,
    to_status TEXT NOT NULL
        CHECK (to_status IN ('planned', 'submitting', 'open', 'partially_filled', 'cancel_pending', 'cancelled', 'filled', 'expired', 'rejected', 'unknown')),
    reason_code TEXT NOT NULL CHECK (length(btrim(reason_code)) > 0),
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata_json) = 'object'),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        from_status IS NULL
        OR from_status IN ('planned', 'submitting', 'open', 'partially_filled', 'cancel_pending', 'cancelled', 'filled', 'expired', 'rejected', 'unknown')
    )
);

CREATE INDEX order_transitions_order_idx
    ON order_transitions (managed_order_id, occurred_at DESC, transition_id DESC);
CREATE INDEX order_transitions_action_idx
    ON order_transitions (action_id, occurred_at DESC) WHERE action_id IS NOT NULL;

CREATE TABLE positions (
    position_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    wallet_id BIGINT NOT NULL,
    market_id BIGINT NOT NULL REFERENCES managed_markets (market_id) ON DELETE RESTRICT,
    token_id TEXT NOT NULL CHECK (length(btrim(token_id)) > 0),
    outcome TEXT NOT NULL CHECK (outcome IN ('yes', 'no')),
    quantity NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (quantity >= 0),
    average_price NUMERIC(12, 10) NOT NULL DEFAULT 0 CHECK (average_price >= 0 AND average_price < 1),
    realized_pnl NUMERIC(24, 8) NOT NULL DEFAULT 0,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    observed_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_id, token_id),
    UNIQUE (owner_user_id, position_id),
    FOREIGN KEY (owner_user_id, wallet_id)
        REFERENCES wallet_accounts (owner_user_id, wallet_id) ON DELETE CASCADE
) WITH (fillfactor = 90);

CREATE INDEX positions_user_idx ON positions (owner_user_id, updated_at DESC);
CREATE INDEX positions_market_idx ON positions (market_id, wallet_id);
CREATE INDEX positions_wallet_nonzero_idx
    ON positions (wallet_id, updated_at DESC) WHERE quantity > 0;

CREATE TABLE venue_fills (
    fill_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    wallet_id BIGINT NOT NULL,
    managed_order_id BIGINT REFERENCES managed_orders (managed_order_id) ON DELETE SET NULL,
    market_id BIGINT NOT NULL REFERENCES managed_markets (market_id) ON DELETE RESTRICT,
    token_id TEXT NOT NULL CHECK (length(btrim(token_id)) > 0),
    external_fill_id TEXT NOT NULL CHECK (length(btrim(external_fill_id)) > 0),
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price NUMERIC(12, 10) NOT NULL CHECK (price > 0 AND price < 1),
    quantity NUMERIC(24, 8) NOT NULL CHECK (quantity > 0),
    fee_amount NUMERIC(24, 8) NOT NULL DEFAULT 0 CHECK (fee_amount >= 0),
    occurred_at TIMESTAMPTZ NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_id, external_fill_id),
    FOREIGN KEY (owner_user_id, wallet_id)
        REFERENCES wallet_accounts (owner_user_id, wallet_id) ON DELETE RESTRICT
);

CREATE INDEX venue_fills_user_time_idx
    ON venue_fills (owner_user_id, occurred_at DESC, fill_id DESC);
CREATE INDEX venue_fills_wallet_time_idx
    ON venue_fills (wallet_id, occurred_at DESC, fill_id DESC);
CREATE INDEX venue_fills_order_idx
    ON venue_fills (managed_order_id, occurred_at DESC) WHERE managed_order_id IS NOT NULL;
CREATE INDEX venue_fills_market_idx ON venue_fills (market_id, occurred_at DESC);

CREATE TABLE external_cash_flows (
    cash_flow_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    wallet_id BIGINT NOT NULL,
    flow_type TEXT NOT NULL
        CHECK (flow_type IN ('deposit', 'withdrawal', 'reward', 'fee', 'adjustment')),
    amount NUMERIC(24, 8) NOT NULL CHECK (amount > 0),
    external_reference TEXT,
    note TEXT CHECK (note IS NULL OR length(note) <= 500),
    occurred_at TIMESTAMPTZ NOT NULL,
    recorded_by_user_id BIGINT REFERENCES users (user_id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (owner_user_id, wallet_id)
        REFERENCES wallet_accounts (owner_user_id, wallet_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX external_cash_flows_reference_uidx
    ON external_cash_flows (wallet_id, external_reference)
    WHERE external_reference IS NOT NULL;
CREATE INDEX external_cash_flows_user_time_idx
    ON external_cash_flows (owner_user_id, occurred_at DESC, cash_flow_id DESC);
CREATE INDEX external_cash_flows_wallet_time_idx
    ON external_cash_flows (wallet_id, occurred_at DESC, cash_flow_id DESC);
CREATE INDEX external_cash_flows_recorder_idx
    ON external_cash_flows (recorded_by_user_id, created_at DESC)
    WHERE recorded_by_user_id IS NOT NULL;

CREATE TABLE position_valuation_snapshots (
    valuation_snapshot_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_user_id BIGINT NOT NULL,
    wallet_id BIGINT NOT NULL,
    position_id BIGINT NOT NULL,
    quantity NUMERIC(24, 8) NOT NULL CHECK (quantity >= 0),
    mark_price NUMERIC(12, 10) CHECK (mark_price IS NULL OR (mark_price >= 0 AND mark_price < 1)),
    market_value NUMERIC(24, 8),
    unrealized_pnl NUMERIC(24, 8),
    valuation_status TEXT NOT NULL CHECK (valuation_status IN ('complete', 'stale', 'unavailable')),
    observed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (owner_user_id, wallet_id)
        REFERENCES wallet_accounts (owner_user_id, wallet_id) ON DELETE CASCADE,
    FOREIGN KEY (owner_user_id, position_id)
        REFERENCES positions (owner_user_id, position_id) ON DELETE CASCADE,
    CHECK (
        (valuation_status = 'complete' AND mark_price IS NOT NULL AND market_value IS NOT NULL AND unrealized_pnl IS NOT NULL AND observed_at IS NOT NULL)
        OR valuation_status <> 'complete'
    )
);

CREATE INDEX position_valuation_snapshots_position_idx
    ON position_valuation_snapshots (position_id, created_at DESC);
CREATE INDEX position_valuation_snapshots_user_idx
    ON position_valuation_snapshots (owner_user_id, created_at DESC);

CREATE TABLE wallet_equity_snapshots (
    equity_snapshot_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_user_id BIGINT NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    wallet_id BIGINT NOT NULL,
    collateral_balance NUMERIC(24, 8) NOT NULL,
    position_market_value NUMERIC(24, 8),
    realized_pnl NUMERIC(24, 8) NOT NULL DEFAULT 0,
    unrealized_pnl NUMERIC(24, 8),
    fee_total NUMERIC(24, 8) NOT NULL DEFAULT 0,
    reward_total NUMERIC(24, 8) NOT NULL DEFAULT 0,
    net_cash_flow NUMERIC(24, 8) NOT NULL DEFAULT 0,
    total_equity NUMERIC(24, 8),
    valuation_status TEXT NOT NULL CHECK (valuation_status IN ('complete', 'partial', 'unavailable')),
    observed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (owner_user_id, wallet_id)
        REFERENCES wallet_accounts (owner_user_id, wallet_id) ON DELETE CASCADE,
    CHECK (
        (valuation_status = 'complete' AND position_market_value IS NOT NULL AND unrealized_pnl IS NOT NULL AND total_equity IS NOT NULL)
        OR valuation_status <> 'complete'
    )
);

CREATE INDEX wallet_equity_snapshots_wallet_idx
    ON wallet_equity_snapshots (wallet_id, observed_at DESC, equity_snapshot_id DESC);
CREATE INDEX wallet_equity_snapshots_user_idx
    ON wallet_equity_snapshots (owner_user_id, observed_at DESC, equity_snapshot_id DESC);

CREATE TABLE idempotency_keys (
    idempotency_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'system')),
    actor_user_id BIGINT REFERENCES users (user_id) ON DELETE RESTRICT,
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
    UNIQUE NULLS NOT DISTINCT (actor_type, actor_user_id, scope, idempotency_key),
    CHECK ((actor_type = 'user') = (actor_user_id IS NOT NULL)),
    CHECK (updated_at >= created_at),
    CHECK (expires_at > created_at),
    CHECK (status <> 'started' OR lease_expires_at IS NOT NULL)
) WITH (fillfactor = 90);

CREATE INDEX idempotency_keys_actor_idx
    ON idempotency_keys (actor_user_id, created_at DESC) WHERE actor_user_id IS NOT NULL;
CREATE INDEX idempotency_keys_claim_idx
    ON idempotency_keys (lease_expires_at, idempotency_id) WHERE status = 'started';
CREATE INDEX idempotency_keys_expiry_idx ON idempotency_keys (expires_at);

CREATE TABLE audit_logs (
    audit_log_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    request_id TEXT NOT NULL CHECK (length(btrim(request_id)) > 0),
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'system')),
    actor_user_id BIGINT REFERENCES users (user_id) ON DELETE RESTRICT,
    actor_session_id UUID REFERENCES user_sessions (session_id) ON DELETE SET NULL,
    action TEXT NOT NULL CHECK (length(btrim(action)) > 0),
    resource_owner_user_id BIGINT REFERENCES users (user_id) ON DELETE RESTRICT,
    resource_type TEXT NOT NULL CHECK (length(btrim(resource_type)) > 0),
    resource_id TEXT NOT NULL CHECK (length(btrim(resource_id)) > 0),
    result TEXT NOT NULL CHECK (result IN ('accepted', 'succeeded', 'rejected', 'failed')),
    operator_note TEXT CHECK (operator_note IS NULL OR length(operator_note) <= 500),
    error_code TEXT,
    payload_json JSONB CHECK (payload_json IS NULL OR jsonb_typeof(payload_json) = 'object'),
    CHECK ((actor_type = 'user') = (actor_user_id IS NOT NULL)),
    CHECK (actor_session_id IS NULL OR actor_user_id IS NOT NULL)
);

CREATE INDEX audit_logs_request_idx ON audit_logs (request_id);
CREATE INDEX audit_logs_actor_idx ON audit_logs (actor_user_id, occurred_at DESC);
CREATE INDEX audit_logs_resource_idx
    ON audit_logs (resource_type, resource_id, occurred_at DESC);
CREATE INDEX audit_logs_resource_owner_idx
    ON audit_logs (resource_owner_user_id, occurred_at DESC)
    WHERE resource_owner_user_id IS NOT NULL;
CREATE INDEX audit_logs_action_idx ON audit_logs (action, occurred_at DESC);

CREATE TABLE system_runtime_state (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    kill_switch_locked BOOLEAN NOT NULL DEFAULT TRUE,
    trading_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    reason TEXT,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    updated_by_user_id BIGINT REFERENCES users (user_id) ON DELETE RESTRICT,
    updated_by_actor TEXT NOT NULL DEFAULT 'system'
        CHECK (updated_by_actor IN ('system', 'user')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (kill_switch_locked = FALSE OR trading_enabled = FALSE),
    CHECK ((updated_by_actor = 'user') = (updated_by_user_id IS NOT NULL))
);

INSERT INTO system_runtime_state (
    singleton,
    kill_switch_locked,
    trading_enabled,
    reason,
    version,
    updated_by_user_id,
    updated_by_actor
) VALUES (
    TRUE,
    TRUE,
    FALSE,
    'clean deploy starts fail-closed',
    0,
    NULL,
    'system'
);
