ALTER TABLE order_drafts
    ADD COLUMN external_order_id TEXT,
    ADD COLUMN submitted_at TIMESTAMPTZ,
    ADD COLUMN failure_code TEXT,
    ADD COLUMN failure_message TEXT;

CREATE INDEX order_drafts_external_order_id_idx
    ON order_drafts (external_order_id)
    WHERE external_order_id IS NOT NULL;

ALTER TABLE execution_requests
    ADD COLUMN external_order_id TEXT,
    ADD COLUMN submitted_at TIMESTAMPTZ,
    ADD COLUMN failure_code TEXT,
    ADD COLUMN failure_message TEXT;

CREATE INDEX execution_requests_external_order_id_idx
    ON execution_requests (external_order_id)
    WHERE external_order_id IS NOT NULL;
