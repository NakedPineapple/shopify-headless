-- Revert users table creation

DROP TRIGGER IF EXISTS user_updated_at ON storefront.user;
DROP TABLE IF EXISTS storefront.user;
DROP FUNCTION IF EXISTS storefront.update_updated_at_column();
