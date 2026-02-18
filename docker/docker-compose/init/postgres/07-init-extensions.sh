#!/bin/bash
set -e

for db in np_storefront np_admin; do
  psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$db" <<-EOSQL
        CREATE EXTENSION IF NOT EXISTS citext;
EOSQL
done

# pgvector extension for embedding-based search
for db in np_storefront np_admin; do
  psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$db" <<-EOSQL
        CREATE EXTENSION IF NOT EXISTS vector;
EOSQL
done

# pg_trgm extension for trigram similarity search (contact graph)
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "np_admin" <<-EOSQL
        CREATE EXTENSION IF NOT EXISTS pg_trgm;
EOSQL
