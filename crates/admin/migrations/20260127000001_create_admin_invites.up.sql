-- Admin invites table for email-verified passkey setup
-- Only emails in this table can register as admins

CREATE TABLE admin.admin_invite (
    id SERIAL PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'admin' CHECK (role IN ('admin', 'super_admin')),
    invited_by INTEGER REFERENCES admin.admin_user(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '7 days'),
    used_at TIMESTAMPTZ,
    used_by INTEGER REFERENCES admin.admin_user(id)
);

-- Index for looking up invites by email
CREATE INDEX idx_admin_invite_email ON admin.admin_invite(email);

-- Index for finding expired/unused invites for cleanup
CREATE INDEX idx_admin_invite_expires ON admin.admin_invite(expires_at) WHERE used_at IS NULL;

COMMENT ON TABLE admin.admin_invite IS 'Email allowlist for admin registration. Invites can only be used once.';
COMMENT ON COLUMN admin.admin_invite.role IS 'Role to assign when invite is used: admin or super_admin';
COMMENT ON COLUMN admin.admin_invite.expires_at IS 'Invite expires 7 days after creation by default';
