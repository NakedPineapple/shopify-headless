SET search_path TO admin, public;

ALTER TABLE admin.inbound_email ADD COLUMN embedding vector(1536);

CREATE INDEX idx_inbound_email_embedding
    ON admin.inbound_email USING hnsw (embedding vector_cosine_ops);
