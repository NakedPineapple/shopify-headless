-- Add email column to user_credential for passkey-by-email lookup.
-- This allows users to authenticate with passkeys using their email address
-- without needing to know their Shopify customer ID upfront.

ALTER TABLE storefront.user_credential
    ADD COLUMN email TEXT;

-- Create index for email lookups during passkey authentication
CREATE INDEX idx_user_credential_email ON storefront.user_credential (email)
    WHERE email IS NOT NULL;

-- Backfill existing credentials if we have email data available
-- (For credentials created before this migration, email will be NULL
-- and users will need to re-register their passkey or login with password first)
