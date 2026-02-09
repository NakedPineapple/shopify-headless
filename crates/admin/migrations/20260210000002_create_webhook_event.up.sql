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

-- Restricted role for public-facing webhook handlers.
--
-- This role can only INSERT and SELECT on admin.webhook_event. It has no
-- access to admin.shopify_token, admin.inbound_email, or any other table.
-- The password must be set via: ALTER ROLE webhook_receiver PASSWORD 'xxx';
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'webhook_receiver') THEN
        CREATE ROLE webhook_receiver LOGIN;
    END IF;
END $$;

GRANT USAGE ON SCHEMA admin TO webhook_receiver;
GRANT INSERT, SELECT ON admin.webhook_event TO webhook_receiver;
GRANT USAGE, SELECT ON SEQUENCE admin.webhook_event_id_seq TO webhook_receiver;

-- Explicitly deny access to sensitive tables.
-- Default is no access, but being explicit documents the security boundary.
REVOKE ALL ON admin.shopify_token FROM webhook_receiver;
