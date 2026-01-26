-- Remove email column from user_credential

DROP INDEX IF EXISTS storefront.idx_user_credential_email;

ALTER TABLE storefront.user_credential
    DROP COLUMN IF EXISTS email;
