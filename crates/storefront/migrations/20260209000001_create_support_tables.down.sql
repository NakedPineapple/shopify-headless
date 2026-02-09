SET search_path TO storefront, public;

DROP TABLE IF EXISTS storefront.support_knowledge;
DROP TABLE IF EXISTS storefront.support_ticket;
DROP TABLE IF EXISTS storefront.support_message;
DROP TABLE IF EXISTS storefront.support_conversation;

DROP TYPE IF EXISTS storefront.support_message_role;
DROP TYPE IF EXISTS storefront.support_conversation_status;
