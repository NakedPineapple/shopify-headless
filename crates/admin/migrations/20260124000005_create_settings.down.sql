-- Revert settings table creation

DROP TRIGGER IF EXISTS settings_updated_at ON admin.settings;
DROP TABLE IF EXISTS admin.settings;
