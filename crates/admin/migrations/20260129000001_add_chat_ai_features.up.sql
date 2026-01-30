-- Add AI chat features: pgvector for tool selection, action queue for Slack confirmations,
-- session metrics for debug panel

SET search_path TO admin, public;

-- Enable pgvector extension for embedding-based tool selection
CREATE EXTENSION IF NOT EXISTS vector;

-- Add API interaction metadata to messages for debug panel
ALTER TABLE admin.chat_message ADD COLUMN api_interaction JSONB NULL;

-- Session-level metrics for debug panel
CREATE TABLE admin.chat_session_metrics (
    id SERIAL PRIMARY KEY,
    chat_session_id INTEGER NOT NULL REFERENCES admin.chat_session(id) ON DELETE CASCADE,
    total_input_tokens INTEGER NOT NULL DEFAULT 0,
    total_output_tokens INTEGER NOT NULL DEFAULT 0,
    total_api_calls INTEGER NOT NULL DEFAULT 0,
    total_tool_calls INTEGER NOT NULL DEFAULT 0,
    total_duration_ms BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    UNIQUE (chat_session_id)
);

CREATE TRIGGER chat_session_metrics_updated_at
    BEFORE UPDATE ON admin.chat_session_metrics
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();

-- Tool example queries for embedding-based selection
-- Many-to-one mapping: multiple example queries can map to same tool
CREATE TABLE admin.tool_example_queries (
    id SERIAL PRIMARY KEY,
    tool_name TEXT NOT NULL,
    domain TEXT NOT NULL,
    example_query TEXT NOT NULL,
    embedding vector(1536) NOT NULL,
    is_learned BOOLEAN NOT NULL DEFAULT FALSE,
    usage_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_tool_examples_tool_name ON admin.tool_example_queries(tool_name);
CREATE INDEX idx_tool_examples_domain ON admin.tool_example_queries(domain);

-- IVFFlat index for fast similarity search
-- lists = 20 is appropriate for ~1000 rows, increase if dataset grows significantly
CREATE INDEX idx_tool_examples_embedding ON admin.tool_example_queries
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 20);

-- Action queue for Slack confirmations of write operations
CREATE TYPE admin.action_status AS ENUM ('pending', 'approved', 'rejected', 'executed', 'failed', 'expired');

CREATE TABLE admin.pending_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chat_session_id INTEGER NOT NULL REFERENCES admin.chat_session(id) ON DELETE CASCADE,
    chat_message_id INTEGER REFERENCES admin.chat_message(id) ON DELETE SET NULL,
    admin_user_id INTEGER NOT NULL REFERENCES admin.admin_user(id) ON DELETE CASCADE,
    tool_name TEXT NOT NULL,
    tool_input JSONB NOT NULL,
    status admin.action_status NOT NULL DEFAULT 'pending',
    slack_message_ts TEXT,
    slack_channel_id TEXT,
    result JSONB,
    error_message TEXT,
    approved_by TEXT,
    rejected_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    resolved_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc' + INTERVAL '1 hour')
);

CREATE INDEX idx_pending_actions_status ON admin.pending_actions(status) WHERE status = 'pending';
CREATE INDEX idx_pending_actions_session ON admin.pending_actions(chat_session_id);
CREATE INDEX idx_pending_actions_admin ON admin.pending_actions(admin_user_id);
CREATE INDEX idx_pending_actions_expires ON admin.pending_actions(expires_at) WHERE status = 'pending';

-- Index for super admin queries (list all sessions across admins)
CREATE INDEX idx_chat_session_admin_created ON admin.chat_session(admin_user_id, created_at DESC);
