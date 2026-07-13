-- Separate scheduled-event timestamps from market lifecycle dates and make
-- source snapshots replaceable without deleting audit history.
ALTER TABLE reward_market_event_windows
    ADD COLUMN event_key TEXT NOT NULL DEFAULT 'legacy'
        CHECK (btrim(event_key) <> ''),
    ADD COLUMN event_time_role TEXT NOT NULL DEFAULT 'unknown'
        CHECK (event_time_role IN (
            'event_occurrence',
            'market_lifecycle',
            'resolution_deadline',
            'unknown'
        )),
    ADD COLUMN schedule_status TEXT NOT NULL DEFAULT 'unknown'
        CHECK (schedule_status IN (
            'scheduled',
            'conflicting',
            'finished',
            'withdrawn',
            'unknown'
        )),
    ADD COLUMN time_precision TEXT NOT NULL DEFAULT 'unknown'
        CHECK (time_precision IN ('exact', 'date_only', 'inferred', 'unknown')),
    ADD COLUMN start_source_field TEXT,
    ADD COLUMN end_policy TEXT NOT NULL DEFAULT 'unknown'
        CHECK (end_policy IN ('explicit', 'point', 'until_market_closed', 'unknown')),
    ADD COLUMN hard_gate_eligible BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN producer_version BIGINT NOT NULL DEFAULT 1
        CHECK (producer_version > 0 AND producer_version <= 4294967295),
    ADD COLUMN source_updated_at TIMESTAMPTZ,
    ADD COLUMN observed_at TIMESTAMPTZ,
    ADD COLUMN expires_at TIMESTAMPTZ;

UPDATE reward_market_event_windows
SET observed_at = updated_at
WHERE observed_at IS NULL;

-- The legacy schema did not record time role, precision or field provenance.
-- Keep all old rows for audit, but never infer hard-gate eligibility from a
-- source label alone. Producers must republish them through the new contract.
UPDATE reward_market_event_windows
SET event_time_role = 'unknown',
    schedule_status = CASE WHEN active THEN 'unknown' ELSE 'withdrawn' END,
    time_precision = 'unknown',
    start_source_field = NULL,
    end_policy = 'unknown',
    hard_gate_eligible = FALSE;

UPDATE reward_market_event_windows
SET active = FALSE,
    hard_gate_eligible = FALSE,
    event_time_role = 'unknown',
    schedule_status = 'withdrawn',
    time_precision = 'unknown',
    end_policy = 'unknown',
    expires_at = COALESCE(expires_at, now()),
    source_payload = source_payload || jsonb_build_object(
        'migration_quarantined', TRUE,
        'migration_quarantined_reason',
        'legacy Gamma row lost event-versus-market date provenance'
    ),
    notes = CASE
        WHEN notes = '' THEN 'Quarantined by event-window semantic migration.'
        ELSE notes || E'\nQuarantined by event-window semantic migration.'
    END,
    updated_at = now()
WHERE source IN ('gamma', 'gamma_reviewed');

ALTER TABLE reward_market_event_windows
    DROP CONSTRAINT reward_market_event_windows_pkey,
    ADD PRIMARY KEY (condition_id, source, event_key),
    ADD CONSTRAINT reward_market_event_windows_hard_gate_shape_ck CHECK (
        NOT hard_gate_eligible
        OR (
            active
            AND event_time_role = 'event_occurrence'
            AND schedule_status = 'scheduled'
            AND time_precision = 'exact'
            AND start_source_field IS NOT NULL
            AND btrim(start_source_field) <> ''
            AND event_start_at IS NOT NULL
            AND end_policy <> 'unknown'
            AND (
                end_policy IN ('point', 'until_market_closed')
                OR event_end_at IS NOT NULL
            )
            AND (
                event_end_at IS NULL
                OR event_end_at >= event_start_at
            )
        )
    ),
    ADD CONSTRAINT reward_market_event_windows_expiry_ck CHECK (
        expires_at IS NULL
        OR observed_at IS NULL
        OR expires_at >= observed_at
    );

ALTER TABLE reward_market_event_windows
    ALTER COLUMN event_key DROP DEFAULT;

DROP INDEX IF EXISTS reward_market_event_windows_active_idx;

CREATE INDEX reward_market_event_windows_active_idx
    ON reward_market_event_windows (
        condition_id,
        expires_at,
        updated_at DESC,
        source,
        event_key
    )
    WHERE active;

CREATE INDEX reward_market_event_windows_source_active_idx
    ON reward_market_event_windows (
        source,
        condition_id,
        observed_at DESC,
        event_key
    )
    WHERE active;

CREATE TABLE reward_event_window_source_versions (
    source TEXT NOT NULL CHECK (source = btrim(source) AND source <> ''),
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    producer_version BIGINT NOT NULL
        CHECK (producer_version > 0 AND producer_version <= 4294967295),
    source_updated_at TIMESTAMPTZ,
    observed_at TIMESTAMPTZ NOT NULL,
    snapshot_hash TEXT NOT NULL
        CHECK (snapshot_hash ~ '^[0-9a-f]{64}$'),
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (source, condition_id)
);

CREATE INDEX reward_event_window_source_versions_observed_idx
    ON reward_event_window_source_versions (
        source,
        source_updated_at DESC,
        observed_at DESC,
        condition_id
    );
