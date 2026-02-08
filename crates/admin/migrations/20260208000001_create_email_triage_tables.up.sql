-- Inbound email tracking and AI classification results.
CREATE TABLE inbound_email (
    id                  SERIAL PRIMARY KEY,
    m365_message_id     TEXT NOT NULL UNIQUE,
    conversation_id     TEXT NOT NULL,
    mailbox             TEXT NOT NULL,
    from_address        TEXT NOT NULL,
    from_name           TEXT,
    to_addresses        JSONB NOT NULL DEFAULT '[]',
    subject             TEXT NOT NULL,
    body_preview        TEXT,
    body_text           TEXT,
    received_at         TIMESTAMPTZ NOT NULL,
    -- Classification (set after AI processes the email)
    classification      TEXT,
    sub_category        TEXT,
    confidence          REAL,
    reasoning           TEXT,
    -- Routing
    status              TEXT NOT NULL DEFAULT 'pending',
    routed_to           TEXT,
    klaviyo_ticket_id   TEXT,
    response_draft      TEXT,
    response_approved   BOOLEAN NOT NULL DEFAULT FALSE,
    response_sent_at    TIMESTAMPTZ,
    -- Metadata
    claude_usage        JSONB,
    reviewed_by         TEXT,
    reviewed_at         TIMESTAMPTZ,
    error               TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_inbound_email_status ON inbound_email(status);
CREATE INDEX idx_inbound_email_conversation ON inbound_email(conversation_id);
CREATE INDEX idx_inbound_email_received ON inbound_email(received_at DESC);
CREATE INDEX idx_inbound_email_from ON inbound_email(from_address);
