-- Rollback admin authentication tables
SET search_path TO admin, public;

-- Drop session table
DROP TABLE IF EXISTS admin.session;
