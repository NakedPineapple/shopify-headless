-- Create per-database users (service user + admin user per database)

-- Storefront database users
CREATE USER storefront_service WITH ENCRYPTED PASSWORD 'storefront_service_password';
CREATE USER storefront_admin WITH ENCRYPTED PASSWORD 'storefront_admin_password';

-- Admin database users
CREATE USER admin_service WITH ENCRYPTED PASSWORD 'admin_service_password';
CREATE USER admin_admin WITH ENCRYPTED PASSWORD 'admin_admin_password';

-- Integration test user
CREATE USER integration_test WITH ENCRYPTED PASSWORD 'integration_test';

-- Service users: CONNECT only (used by applications)
GRANT CONNECT ON DATABASE np_storefront TO storefront_service;
GRANT CONNECT ON DATABASE np_admin TO admin_service;

-- Admin users: Own the database (used for migrations)
GRANT CONNECT, TEMPORARY, CREATE ON DATABASE np_storefront TO storefront_admin;
ALTER DATABASE np_storefront OWNER TO storefront_admin;
GRANT CONNECT, TEMPORARY, CREATE ON DATABASE np_admin TO admin_admin;
ALTER DATABASE np_admin OWNER TO admin_admin;

-- Integration test: Full access to all databases
GRANT ALL PRIVILEGES ON DATABASE np_storefront TO integration_test;
GRANT ALL PRIVILEGES ON DATABASE np_admin TO integration_test;
