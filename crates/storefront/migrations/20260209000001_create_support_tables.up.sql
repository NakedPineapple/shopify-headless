SET search_path TO storefront, public;

-- Support conversation status
CREATE TYPE storefront.support_conversation_status AS ENUM (
    'active', 'escalated', 'waiting', 'resolved', 'closed'
);

-- Support message role
CREATE TYPE storefront.support_message_role AS ENUM (
    'customer', 'assistant', 'agent', 'system', 'tool_use', 'tool_result'
);

-- Support conversations
CREATE TABLE storefront.support_conversation (
    id                       SERIAL PRIMARY KEY,
    session_token            TEXT NOT NULL,
    shopify_customer_id      TEXT,
    customer_email           TEXT,
    customer_name            TEXT,
    status                   storefront.support_conversation_status NOT NULL DEFAULT 'active',
    assigned_admin_id        INTEGER,
    escalated_at             TIMESTAMPTZ,
    escalation_reason        TEXT,
    title                    TEXT,
    is_authenticated         BOOLEAN NOT NULL DEFAULT FALSE,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    resolved_at              TIMESTAMPTZ,
    last_customer_message_at TIMESTAMPTZ,
    last_agent_message_at    TIMESTAMPTZ
);

CREATE INDEX idx_support_conversation_status ON storefront.support_conversation(status);
CREATE INDEX idx_support_conversation_session ON storefront.support_conversation(session_token);
CREATE INDEX idx_support_conversation_customer ON storefront.support_conversation(shopify_customer_id);
CREATE INDEX idx_support_conversation_escalated ON storefront.support_conversation(escalated_at DESC)
    WHERE status = 'escalated';

CREATE TRIGGER support_conversation_updated_at
    BEFORE UPDATE ON storefront.support_conversation
    FOR EACH ROW
    EXECUTE FUNCTION storefront.update_updated_at_column();

-- Support messages
CREATE TABLE storefront.support_message (
    id                      SERIAL PRIMARY KEY,
    support_conversation_id INTEGER NOT NULL REFERENCES storefront.support_conversation(id) ON DELETE CASCADE,
    role                    storefront.support_message_role NOT NULL,
    content                 JSONB NOT NULL,
    api_interaction         JSONB,
    admin_user_id           INTEGER,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_support_message_conversation ON storefront.support_message(support_conversation_id, created_at ASC);

-- Support tickets
CREATE TABLE storefront.support_ticket (
    id                      SERIAL PRIMARY KEY,
    support_conversation_id INTEGER NOT NULL REFERENCES storefront.support_conversation(id),
    category                TEXT,
    priority                TEXT NOT NULL DEFAULT 'normal',
    status                  TEXT NOT NULL DEFAULT 'open',
    assigned_admin_id       INTEGER,
    resolution_notes        TEXT,
    slack_message_ts        TEXT,
    slack_channel_id        TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    resolved_at             TIMESTAMPTZ
);

CREATE INDEX idx_support_ticket_status ON storefront.support_ticket(status);
CREATE INDEX idx_support_ticket_assigned ON storefront.support_ticket(assigned_admin_id);
CREATE INDEX idx_support_ticket_conversation ON storefront.support_ticket(support_conversation_id);

CREATE TRIGGER support_ticket_updated_at
    BEFORE UPDATE ON storefront.support_ticket
    FOR EACH ROW
    EXECUTE FUNCTION storefront.update_updated_at_column();

-- Knowledge base for RAG (requires pgvector extension, installed via docker init script)
CREATE TABLE storefront.support_knowledge (
    id              SERIAL PRIMARY KEY,
    title           TEXT NOT NULL,
    content         TEXT NOT NULL,
    category        TEXT NOT NULL,
    embedding       vector(1536) NOT NULL,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_by      INTEGER,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_support_knowledge_category ON storefront.support_knowledge(category);
CREATE INDEX idx_support_knowledge_active ON storefront.support_knowledge(is_active) WHERE is_active = TRUE;
CREATE INDEX idx_support_knowledge_embedding ON storefront.support_knowledge
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 10);

CREATE TRIGGER support_knowledge_updated_at
    BEFORE UPDATE ON storefront.support_knowledge
    FOR EACH ROW
    EXECUTE FUNCTION storefront.update_updated_at_column();
