-- Create batch_metadata table for key-value metadata on manufacturing batches

SET search_path TO admin, public;

CREATE TABLE admin.batch_metadata (
    id SERIAL PRIMARY KEY,
    batch_id INTEGER NOT NULL REFERENCES admin.manufacturing_batch(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_batch_metadata_batch_id ON admin.batch_metadata(batch_id);
CREATE UNIQUE INDEX idx_batch_metadata_batch_key ON admin.batch_metadata(batch_id, key);
