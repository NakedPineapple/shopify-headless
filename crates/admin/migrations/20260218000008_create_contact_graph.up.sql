SET search_path TO admin, public;

-- pg_trgm for trigram similarity search on contact names
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Nodes: people and organizations in the contact graph
CREATE TABLE admin.contacts (
    id              SERIAL PRIMARY KEY,
    contact_type    TEXT NOT NULL CHECK (contact_type IN ('person', 'organization')),
    name            TEXT NOT NULL,
    email           CITEXT,
    domain          CITEXT,
    metadata        JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_contacts_email ON admin.contacts(email);
CREATE INDEX idx_contacts_domain ON admin.contacts(domain);
CREATE INDEX idx_contacts_type ON admin.contacts(contact_type);
CREATE INDEX idx_contacts_name_trgm ON admin.contacts USING gin (name gin_trgm_ops);

CREATE TRIGGER contacts_updated_at
    BEFORE UPDATE ON admin.contacts
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();

-- Edges: relationships between contacts
CREATE TABLE admin.contact_relationships (
    id                  SERIAL PRIMARY KEY,
    from_contact_id     INT NOT NULL REFERENCES admin.contacts(id) ON DELETE CASCADE,
    to_contact_id       INT NOT NULL REFERENCES admin.contacts(id) ON DELETE CASCADE,
    relationship_type   TEXT NOT NULL,
    properties          JSONB NOT NULL DEFAULT '{}',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    CONSTRAINT no_self_relationship CHECK (from_contact_id != to_contact_id),
    CONSTRAINT unique_relationship UNIQUE (from_contact_id, to_contact_id, relationship_type)
);

CREATE INDEX idx_relationships_from ON admin.contact_relationships(from_contact_id);
CREATE INDEX idx_relationships_to ON admin.contact_relationships(to_contact_id);
CREATE INDEX idx_relationships_type ON admin.contact_relationships(relationship_type);

CREATE TRIGGER contact_relationships_updated_at
    BEFORE UPDATE ON admin.contact_relationships
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();

-- Contribution tracking: which emails contributed to which graph entities
CREATE TABLE admin.email_contact_contributions (
    email_id    INT NOT NULL REFERENCES admin.inbound_email(id) ON DELETE CASCADE,
    contact_id  INT NOT NULL REFERENCES admin.contacts(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    PRIMARY KEY (email_id, contact_id)
);

CREATE TABLE admin.email_relationship_contributions (
    email_id        INT NOT NULL REFERENCES admin.inbound_email(id) ON DELETE CASCADE,
    relationship_id INT NOT NULL REFERENCES admin.contact_relationships(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    PRIMARY KEY (email_id, relationship_id)
);
