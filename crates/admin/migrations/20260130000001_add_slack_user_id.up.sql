-- Add slack_user_id for per-admin Slack DM routing
-- Slack user IDs start with 'U' followed by alphanumeric characters (e.g., U0123456789)

SET search_path TO admin, public;

ALTER TABLE admin.admin_user
ADD COLUMN slack_user_id TEXT;

-- Validate Slack user ID format: starts with U, followed by uppercase letters and digits
ALTER TABLE admin.admin_user
ADD CONSTRAINT chk_slack_user_id_format
CHECK (slack_user_id IS NULL OR slack_user_id ~ '^U[A-Z0-9]+$');
