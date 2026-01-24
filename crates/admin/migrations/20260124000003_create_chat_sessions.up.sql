-- Create chat_session table for Claude AI chat sessions
-- Groups messages by admin user

SET search_path TO admin, public;

CREATE TABLE admin.chat_session (
    id SERIAL PRIMARY KEY,
    admin_user_id INTEGER NOT NULL REFERENCES admin.admin_user(id) ON DELETE CASCADE,
    title TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_chat_session_admin_user_id ON admin.chat_session(admin_user_id);
CREATE INDEX idx_chat_session_updated_at ON admin.chat_session(updated_at DESC);

CREATE TRIGGER chat_session_updated_at
    BEFORE UPDATE ON admin.chat_session
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();
