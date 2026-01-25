-- Revert shopify_customer_id addition to user_credential

SET search_path TO storefront, public;

DROP INDEX IF EXISTS storefront.idx_user_credential_shopify_customer_id;
ALTER TABLE storefront.user_credential DROP COLUMN IF EXISTS shopify_customer_id;
