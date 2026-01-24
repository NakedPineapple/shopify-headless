-- Revert user and user_password tables

DROP TRIGGER IF EXISTS user_password_updated_at ON storefront.user_password;
DROP TABLE IF EXISTS storefront.user_password;
DROP TRIGGER IF EXISTS user_updated_at ON storefront.user;
DROP TABLE IF EXISTS storefront.user;
DROP FUNCTION IF EXISTS storefront.update_updated_at_column();
