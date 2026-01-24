#!/bin/bash
set -e

for db in np_storefront np_admin; do
  psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$db" <<-EOSQL
        CREATE EXTENSION IF NOT EXISTS citext;
EOSQL
done
