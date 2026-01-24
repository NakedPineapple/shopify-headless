#!/bin/bash
set -e

# Grant integration_test user access to all schemas

# Storefront service
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "np_storefront" <<-EOSQL
    GRANT ALL PRIVILEGES ON SCHEMA "storefront" TO integration_test;
    SET ROLE storefront_admin;
    GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA "storefront" TO integration_test;
    ALTER DEFAULT PRIVILEGES IN SCHEMA "storefront"
        GRANT ALL PRIVILEGES ON TABLES TO integration_test;
EOSQL

# Admin service
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "np_admin" <<-EOSQL
    GRANT ALL PRIVILEGES ON SCHEMA "admin" TO integration_test;
    SET ROLE admin_admin;
    GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA "admin" TO integration_test;
    ALTER DEFAULT PRIVILEGES IN SCHEMA "admin"
        GRANT ALL PRIVILEGES ON TABLES TO integration_test;
EOSQL
