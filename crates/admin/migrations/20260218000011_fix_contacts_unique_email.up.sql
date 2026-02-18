SET search_path TO admin, public;

-- pg_trgm may not exist yet if the init script didn't run
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Replace the non-unique email index with a unique partial index
-- required for ON CONFLICT (email) WHERE email IS NOT NULL
DROP INDEX IF EXISTS admin.idx_contacts_email;
CREATE UNIQUE INDEX idx_contacts_email ON admin.contacts(email) WHERE email IS NOT NULL;
