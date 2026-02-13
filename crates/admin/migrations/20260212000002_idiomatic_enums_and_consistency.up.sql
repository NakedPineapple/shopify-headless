-- Convert TEXT+CHECK columns to proper PostgreSQL enums and fix consistency issues.
--
-- 1. expense.recurrence_interval: TEXT CHECK → enum
-- 2. admin_invite.role: TEXT CHECK → reuse existing admin.admin_role enum
-- 3. admin_invite.email: TEXT → CITEXT (matches admin_user.email)
-- 4. admin_invite timestamps: NOW() → CURRENT_TIMESTAMP AT TIME ZONE 'utc'
-- 5. expense: add CHECK for is_recurring ↔ recurrence_interval consistency

SET search_path TO admin, public;

-- 1. Create recurrence_interval enum and migrate column
CREATE TYPE admin.recurrence_interval AS ENUM ('monthly', 'quarterly', 'yearly');

ALTER TABLE admin.expense
    DROP CONSTRAINT expense_recurrence_interval_check,
    ALTER COLUMN recurrence_interval TYPE admin.recurrence_interval
        USING recurrence_interval::admin.recurrence_interval;

-- 2. Migrate admin_invite.role from TEXT to admin.admin_role enum
--    Must drop the TEXT default before changing type, then re-set as enum default.
ALTER TABLE admin.admin_invite
    DROP CONSTRAINT admin_invite_role_check,
    ALTER COLUMN role DROP DEFAULT,
    ALTER COLUMN role TYPE admin.admin_role USING role::admin.admin_role,
    ALTER COLUMN role SET DEFAULT 'admin'::admin.admin_role;

-- 3. admin_invite.email: TEXT → CITEXT for case-insensitive matching
ALTER TABLE admin.admin_invite
    ALTER COLUMN email TYPE CITEXT;

-- 4. Fix timestamp defaults to match project convention
ALTER TABLE admin.admin_invite
    ALTER COLUMN created_at SET DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    ALTER COLUMN expires_at SET DEFAULT ((CURRENT_TIMESTAMP AT TIME ZONE 'utc') + INTERVAL '7 days');

-- 5. Recurring expenses must have an interval; non-recurring must not
ALTER TABLE admin.expense
    ADD CONSTRAINT expense_recurrence_consistency
        CHECK (
            (is_recurring = true AND recurrence_interval IS NOT NULL)
            OR (is_recurring = false AND recurrence_interval IS NULL)
        );
