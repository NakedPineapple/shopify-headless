-- Drop local auth tables (password auth + WebAuthn passkeys).
-- Authentication is now handled exclusively via Shopify Customer Account OAuth.
--
-- Tables dropped (in FK-safe order):
--   1. email_verification_code  (references user)
--   2. password_reset_token     (references user)
--   3. user_credential          (references user)
--   4. user_password            (references user)
--   5. user                     (parent table)
--
-- The update_updated_at_column() trigger function is preserved because
-- shopify_cart_cache still uses it.

SET search_path TO storefront, public;

DROP TABLE IF EXISTS storefront.email_verification_code;
DROP TABLE IF EXISTS storefront.password_reset_token;
DROP TABLE IF EXISTS storefront.user_credential;
DROP TABLE IF EXISTS storefront.user_password;
DROP TABLE IF EXISTS storefront.user;
