-- Automation run log for tracking workflow executions.

SET search_path TO admin, public;

CREATE TABLE admin.automation_log (
    id              BIGSERIAL PRIMARY KEY,
    workflow        TEXT NOT NULL,
    status          TEXT NOT NULL,
    items_processed INTEGER NOT NULL DEFAULT 0,
    items_actioned  INTEGER NOT NULL DEFAULT 0,
    error           TEXT,
    metadata        JSONB,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    completed_at    TIMESTAMPTZ,
    duration_ms     BIGINT
);

CREATE INDEX idx_automation_log_workflow ON admin.automation_log(workflow, started_at DESC);

-- Abandoned cart tracking for recovery workflows.
CREATE TABLE admin.abandoned_cart (
    id                  SERIAL PRIMARY KEY,
    shopify_checkout_id TEXT NOT NULL UNIQUE,
    customer_email      TEXT,
    cart_total          DECIMAL(10,2),
    line_items          JSONB NOT NULL DEFAULT '[]',
    abandoned_at        TIMESTAMPTZ NOT NULL,
    recovery_status     TEXT NOT NULL DEFAULT 'detected',
    first_email_at      TIMESTAMPTZ,
    recovered_at        TIMESTAMPTZ,
    recovery_order_id   TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_abandoned_cart_status ON admin.abandoned_cart(recovery_status);
CREATE INDEX idx_abandoned_cart_abandoned ON admin.abandoned_cart(abandoned_at);

CREATE TRIGGER abandoned_cart_updated_at
    BEFORE UPDATE ON admin.abandoned_cart
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();

-- Outbound email queue for transactional and automated emails.
CREATE TABLE admin.outbound_email_queue (
    id              BIGSERIAL PRIMARY KEY,
    email_type      TEXT NOT NULL,
    to_address      TEXT NOT NULL,
    to_name         TEXT,
    subject         TEXT NOT NULL,
    body_html       TEXT NOT NULL,
    body_text       TEXT NOT NULL,
    scheduled_for   TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    status          TEXT NOT NULL DEFAULT 'queued',
    attempts        INTEGER NOT NULL DEFAULT 0,
    max_attempts    INTEGER NOT NULL DEFAULT 3,
    last_attempt_at TIMESTAMPTZ,
    sent_at         TIMESTAMPTZ,
    error_message   TEXT,
    reference_id    TEXT,
    reference_type  TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_outbound_queue_pending
    ON admin.outbound_email_queue(status, scheduled_for)
    WHERE status = 'queued';
