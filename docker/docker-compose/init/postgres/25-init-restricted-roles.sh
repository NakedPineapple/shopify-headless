#!/bin/bash
set -e

# Create restricted-privilege roles used by specific subsystems.
# These roles get minimal permissions via ALTER DEFAULT PRIVILEGES so that
# grants apply automatically as migrations create tables.

# webhook_receiver: Used by the public-facing webhook listener in the
# automations crate. Can only INSERT and SELECT on admin tables/sequences.
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "np_admin" <<-EOSQL
    DO \$\$
    BEGIN
        IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'webhook_receiver') THEN
            CREATE ROLE webhook_receiver LOGIN;
        END IF;
    END \$\$;

    GRANT USAGE ON SCHEMA admin TO webhook_receiver;

    -- Default privileges so future tables created by admin_admin get grants automatically
    SET ROLE admin_admin;
    ALTER DEFAULT PRIVILEGES IN SCHEMA admin
        GRANT SELECT, INSERT ON TABLES TO webhook_receiver;
    ALTER DEFAULT PRIVILEGES IN SCHEMA admin
        GRANT USAGE, SELECT ON SEQUENCES TO webhook_receiver;
    RESET ROLE;
EOSQL

# support_agent: Used by the admin binary and automations crate to access
# customer support tables in the storefront database. Can read/write
# support_conversation, support_message, support_ticket, and support_knowledge
# but has no access to other storefront tables (user sessions, search index, etc).
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "np_storefront" <<-EOSQL
    DO \$\$
    BEGIN
        IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'support_agent') THEN
            CREATE ROLE support_agent LOGIN;
        END IF;
    END \$\$;

    GRANT USAGE ON SCHEMA storefront TO support_agent;

    -- Default privileges so future tables created by storefront_admin get grants automatically
    SET ROLE storefront_admin;
    ALTER DEFAULT PRIVILEGES IN SCHEMA storefront
        GRANT SELECT, INSERT, UPDATE ON TABLES TO support_agent;
    ALTER DEFAULT PRIVILEGES IN SCHEMA storefront
        GRANT USAGE, SELECT ON SEQUENCES TO support_agent;
    RESET ROLE;
EOSQL
