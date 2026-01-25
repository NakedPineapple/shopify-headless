-- Add shopify_customer_id to user_credential table
-- This allows passkeys to be linked to Shopify customers instead of local users
-- The user_id column is preserved for backwards compatibility during migration

SET search_path TO storefront, public;

-- Add shopify_customer_id column (nullable initially to allow migration)
ALTER TABLE storefront.user_credential
    ADD COLUMN shopify_customer_id TEXT;

-- Create index for looking up credentials by Shopify customer ID
CREATE INDEX idx_user_credential_shopify_customer_id
    ON storefront.user_credential(shopify_customer_id)
    WHERE shopify_customer_id IS NOT NULL;

-- Note: After migrating existing users, you can:
-- 1. Make shopify_customer_id NOT NULL
-- 2. Drop the user_id column and its foreign key
-- This is done in a separate migration to allow data migration first
