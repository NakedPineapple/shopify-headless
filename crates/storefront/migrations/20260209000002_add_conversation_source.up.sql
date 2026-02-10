SET search_path TO storefront, public;

-- Add source column to distinguish chat-originated vs email-originated conversations.
ALTER TABLE storefront.support_conversation
    ADD COLUMN source TEXT NOT NULL DEFAULT 'chat';
