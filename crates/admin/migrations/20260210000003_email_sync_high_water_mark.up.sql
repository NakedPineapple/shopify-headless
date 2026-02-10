-- Per-mailbox sync state and folder/read tracking for inbound emails.

SET search_path TO admin, public;

-- Per-mailbox sync state: stores the receivedDateTime of the newest message seen.
CREATE TABLE admin.email_sync_state (
    id              SERIAL PRIMARY KEY,
    mailbox         TEXT NOT NULL UNIQUE,
    high_water_mark TIMESTAMPTZ NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER email_sync_state_updated_at
    BEFORE UPDATE ON admin.email_sync_state
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();

-- Track which folder the email came from and its read status at sync time.
ALTER TABLE admin.inbound_email
    ADD COLUMN folder  TEXT,
    ADD COLUMN is_read BOOLEAN NOT NULL DEFAULT FALSE;
