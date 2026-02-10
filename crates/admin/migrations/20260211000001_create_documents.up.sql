-- Document upload and embedding storage for AI assistant knowledge base.

SET search_path TO admin, public;

-- Uploaded business documents (PDFs, text files, markdown)
CREATE TABLE admin.documents (
    id              SERIAL PRIMARY KEY,
    filename        TEXT NOT NULL,
    content_type    TEXT NOT NULL,
    file_size       BIGINT NOT NULL,
    r2_key          TEXT NOT NULL,
    uploaded_by     INTEGER NOT NULL REFERENCES admin.admin_user(id) ON DELETE SET NULL,
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_documents_created_at ON admin.documents(created_at DESC);
CREATE INDEX idx_documents_uploaded_by ON admin.documents(uploaded_by);

CREATE TRIGGER documents_updated_at
    BEFORE UPDATE ON admin.documents
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();

-- Text chunks from documents with embeddings for semantic search
CREATE TABLE admin.document_chunks (
    id              SERIAL PRIMARY KEY,
    document_id     INTEGER NOT NULL REFERENCES admin.documents(id) ON DELETE CASCADE,
    chunk_index     INTEGER NOT NULL,
    chunk_text      TEXT NOT NULL,
    token_count     INTEGER NOT NULL,
    embedding       vector(1536) NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),

    UNIQUE(document_id, chunk_index)
);

CREATE INDEX idx_document_chunks_document_id ON admin.document_chunks(document_id);

-- IVFFlat index for fast similarity search
-- lists = 50 is appropriate for up to ~5000 chunks; increase if dataset grows significantly
CREATE INDEX idx_document_chunks_embedding ON admin.document_chunks
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 50);
