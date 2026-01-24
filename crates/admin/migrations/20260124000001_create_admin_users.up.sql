-- Create admin_user table for admin panel authentication
-- WebAuthn-only authentication (no passwords)

SET search_path TO admin, public;

CREATE TYPE admin.admin_role AS ENUM ('super_admin', 'admin', 'viewer');

CREATE TABLE admin.admin_user (
    id SERIAL PRIMARY KEY,
    email CITEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    role admin.admin_role NOT NULL DEFAULT 'viewer',
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_admin_user_email ON admin.admin_user(email);

-- Trigger to auto-update updated_at
CREATE OR REPLACE FUNCTION admin.update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER admin_user_updated_at
    BEFORE UPDATE ON admin.admin_user
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();
