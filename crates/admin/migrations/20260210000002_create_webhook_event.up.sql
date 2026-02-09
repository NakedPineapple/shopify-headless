SET search_path TO admin, public;

-- Webhook event ingestion table.
--
-- All inbound webhooks (Shopify, GitHub, Sentry, Fly.io, Better Stack) are
-- written here first by the public-facing listener using a restricted DB role,
-- then processed asynchronously by the scheduler loop which has full privileges.
CREATE TABLE admin.webhook_event (
    id              BIGSERIAL       PRIMARY KEY,
    source          TEXT            NOT NULL,
    event_type      TEXT            NOT NULL,
    external_id     TEXT,
    payload         JSONB           NOT NULL,
    status          TEXT            NOT NULL DEFAULT 'pending',
    error_message   TEXT,
    received_at     TIMESTAMPTZ     NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    processed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

-- Idempotency: prevent duplicate webhook deliveries from the same source.
CREATE UNIQUE INDEX idx_webhook_event_dedup
    ON admin.webhook_event (source, external_id)
    WHERE external_id IS NOT NULL;

-- Fast lookup for unprocessed events by the scheduler.
CREATE INDEX idx_webhook_event_pending
    ON admin.webhook_event (status, received_at)
    WHERE status = 'pending';

