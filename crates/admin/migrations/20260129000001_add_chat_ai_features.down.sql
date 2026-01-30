-- Rollback AI chat features

SET search_path TO admin, public;

-- Drop pending actions
DROP INDEX IF EXISTS idx_pending_actions_expires;
DROP INDEX IF EXISTS idx_pending_actions_admin;
DROP INDEX IF EXISTS idx_pending_actions_session;
DROP INDEX IF EXISTS idx_pending_actions_status;
DROP TABLE IF EXISTS admin.pending_actions;
DROP TYPE IF EXISTS admin.action_status;

-- Drop tool example queries
DROP INDEX IF EXISTS idx_tool_examples_embedding;
DROP INDEX IF EXISTS idx_tool_examples_domain;
DROP INDEX IF EXISTS idx_tool_examples_tool_name;
DROP TABLE IF EXISTS admin.tool_example_queries;

-- Drop session metrics
DROP TRIGGER IF EXISTS chat_session_metrics_updated_at ON admin.chat_session_metrics;
DROP TABLE IF EXISTS admin.chat_session_metrics;

-- Remove API interaction column
ALTER TABLE admin.chat_message DROP COLUMN IF EXISTS api_interaction;

-- Drop super admin index
DROP INDEX IF EXISTS idx_chat_session_admin_created;

-- Note: We don't drop the pgvector extension as other things may depend on it
