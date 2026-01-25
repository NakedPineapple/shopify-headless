SET search_path TO admin, public;

DROP TRIGGER IF EXISTS shopify_tokens_updated_at ON admin.shopify_token;
DROP TABLE IF EXISTS admin.shopify_token;
