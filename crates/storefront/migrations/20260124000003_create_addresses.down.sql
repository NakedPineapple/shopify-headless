-- Revert addresses table creation

DROP TRIGGER IF EXISTS address_updated_at ON storefront.address;
DROP TABLE IF EXISTS storefront.address;
