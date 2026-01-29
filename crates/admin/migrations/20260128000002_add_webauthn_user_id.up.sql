-- Add webauthn_user_id for discoverable credentials (passkey login without email)
-- This UUID is stored in the passkey and returned during authentication to identify the user

SET search_path TO admin, public;

ALTER TABLE admin.admin_user
ADD COLUMN webauthn_user_id UUID NOT NULL DEFAULT gen_random_uuid();

-- Ensure uniqueness
CREATE UNIQUE INDEX idx_admin_user_webauthn_user_id ON admin.admin_user(webauthn_user_id);
