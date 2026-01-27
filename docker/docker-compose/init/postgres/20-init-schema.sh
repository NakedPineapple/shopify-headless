#!/bin/bash
set -e

# Connect as postgres superuser to create schemas and set up grants

# Storefront service schema
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "np_storefront" <<-EOSQL
    CREATE SCHEMA "storefront" AUTHORIZATION storefront_service;
    ALTER ROLE storefront_service SET search_path TO "storefront", public;
    GRANT ALL PRIVILEGES ON SCHEMA "storefront" TO storefront_admin;
    SET ROLE storefront_admin;
    GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA "storefront" TO storefront_service;
    ALTER DEFAULT PRIVILEGES IN SCHEMA "storefront"
        GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO storefront_service;
    ALTER DEFAULT PRIVILEGES IN SCHEMA "storefront"
        GRANT USAGE, SELECT ON SEQUENCES TO storefront_service;
    RESET ROLE;
    -- Grant public schema access for sqlx _sqlx_migrations table
    GRANT USAGE, CREATE ON SCHEMA public TO storefront_admin;
EOSQL

# Admin service schema
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "np_admin" <<-EOSQL
    CREATE SCHEMA "admin" AUTHORIZATION admin_service;
    ALTER ROLE admin_service SET search_path TO "admin", public;
    GRANT ALL PRIVILEGES ON SCHEMA "admin" TO admin_admin;
    SET ROLE admin_admin;
    GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA "admin" TO admin_service;
    ALTER DEFAULT PRIVILEGES IN SCHEMA "admin"
        GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO admin_service;
    ALTER DEFAULT PRIVILEGES IN SCHEMA "admin"
        GRANT USAGE, SELECT ON SEQUENCES TO admin_service;
    RESET ROLE;
    -- Grant public schema access for sqlx _sqlx_migrations table
    GRANT USAGE, CREATE ON SCHEMA public TO admin_admin;
EOSQL
