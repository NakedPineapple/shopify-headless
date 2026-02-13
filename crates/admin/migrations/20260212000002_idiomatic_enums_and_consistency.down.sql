-- Revert enum columns back to TEXT+CHECK and undo consistency fixes.

SET search_path TO admin, public;

-- 5. Drop recurrence consistency constraint
ALTER TABLE admin.expense
    DROP CONSTRAINT IF EXISTS expense_recurrence_consistency;

-- 4. Revert timestamp defaults to NOW()
ALTER TABLE admin.admin_invite
    ALTER COLUMN created_at SET DEFAULT NOW(),
    ALTER COLUMN expires_at SET DEFAULT (NOW() + INTERVAL '7 days');

-- 3. Revert email back to TEXT
ALTER TABLE admin.admin_invite
    ALTER COLUMN email TYPE TEXT;

-- 2. Revert admin_invite.role from enum to TEXT+CHECK
ALTER TABLE admin.admin_invite
    ALTER COLUMN role TYPE TEXT USING role::TEXT,
    ALTER COLUMN role SET DEFAULT 'admin',
    ADD CONSTRAINT admin_invite_role_check CHECK (role IN ('admin', 'super_admin'));

-- 1. Revert expense.recurrence_interval from enum to TEXT+CHECK
ALTER TABLE admin.expense
    ALTER COLUMN recurrence_interval TYPE TEXT USING recurrence_interval::TEXT,
    ADD CONSTRAINT expense_recurrence_interval_check
        CHECK (recurrence_interval IN ('monthly', 'quarterly', 'yearly'));

DROP TYPE IF EXISTS admin.recurrence_interval;
