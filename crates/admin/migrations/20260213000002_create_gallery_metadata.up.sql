SET search_path TO admin, public;

CREATE TABLE admin.gallery_metadata (
    r2_key          TEXT PRIMARY KEY,
    alt_text        TEXT,
    description     TEXT,
    custom_metadata JSONB NOT NULL DEFAULT '{}',
    updated_by      INTEGER REFERENCES admin.admin_user(id) ON DELETE SET NULL,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER gallery_metadata_updated_at
    BEFORE UPDATE ON admin.gallery_metadata
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();
