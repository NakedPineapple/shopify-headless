-- Revert chat_session table creation

DROP TRIGGER IF EXISTS chat_session_updated_at ON admin.chat_session;
DROP TABLE IF EXISTS admin.chat_session;
