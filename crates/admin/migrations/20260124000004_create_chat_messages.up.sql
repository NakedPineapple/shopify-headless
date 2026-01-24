-- Create chat_message table for Claude AI chat message history
-- Supports tool use with JSONB content

SET search_path TO admin, public;

CREATE TYPE admin.chat_role AS ENUM ('user', 'assistant', 'tool_use', 'tool_result');

CREATE TABLE admin.chat_message (
    id SERIAL PRIMARY KEY,
    chat_session_id INTEGER NOT NULL REFERENCES admin.chat_session(id) ON DELETE CASCADE,
    role admin.chat_role NOT NULL,
    content JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_chat_message_chat_session_id ON admin.chat_message(chat_session_id);
