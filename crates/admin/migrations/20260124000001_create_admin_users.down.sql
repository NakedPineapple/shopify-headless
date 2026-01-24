-- Revert admin_user table creation

DROP TRIGGER IF EXISTS admin_user_updated_at ON admin.admin_user;
DROP TABLE IF EXISTS admin.admin_user;
DROP TYPE IF EXISTS admin.admin_role;
DROP FUNCTION IF EXISTS admin.update_updated_at_column();
