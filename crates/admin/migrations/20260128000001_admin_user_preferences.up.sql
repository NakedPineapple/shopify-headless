-- Add user-specific settings support
-- NULL admin_user_id = global setting, non-NULL = user-specific setting

SET search_path TO admin, public;

-- Add admin_user_id column for user-specific settings
ALTER TABLE admin.settings
ADD COLUMN admin_user_id INTEGER REFERENCES admin.admin_user(id) ON DELETE CASCADE;

-- Drop the existing unique constraint on key alone
ALTER TABLE admin.settings DROP CONSTRAINT settings_key_key;

-- Add composite unique constraint allowing same key for different users
-- NULLS NOT DISTINCT ensures (key, NULL) is also unique (only one global setting per key)
ALTER TABLE admin.settings
ADD CONSTRAINT settings_key_user_unique UNIQUE NULLS NOT DISTINCT (key, admin_user_id);

-- Index for efficient user preference lookups
CREATE INDEX idx_settings_admin_user_id ON admin.settings(admin_user_id)
WHERE admin_user_id IS NOT NULL;

-- Note: idx_settings_key already exists for key lookups
-- Global settings: admin_user_id = NULL
-- User settings: admin_user_id = <user_id>
