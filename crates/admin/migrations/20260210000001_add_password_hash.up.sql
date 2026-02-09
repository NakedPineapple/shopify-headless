-- Add optional password hash for break-glass emergency login.
-- Most admin users will NOT have a password; passkeys remain primary auth.
-- Passwords are hashed with Argon2id and stored as PHC-format strings.

SET search_path TO admin, public;

ALTER TABLE admin.admin_user
ADD COLUMN password_hash TEXT;
