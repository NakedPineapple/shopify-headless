-- Revert user-specific settings support

SET search_path TO admin, public;

-- Drop the user-specific index
DROP INDEX IF EXISTS admin.idx_settings_admin_user_id;

-- Drop the composite unique constraint
ALTER TABLE admin.settings DROP CONSTRAINT IF EXISTS settings_key_user_unique;

-- Restore the original unique constraint on key
ALTER TABLE admin.settings ADD CONSTRAINT settings_key_key UNIQUE (key);

-- Remove the admin_user_id column (this will delete all user-specific settings)
ALTER TABLE admin.settings DROP COLUMN IF EXISTS admin_user_id;
