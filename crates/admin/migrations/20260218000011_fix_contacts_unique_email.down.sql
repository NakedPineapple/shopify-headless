SET search_path TO admin, public;

DROP INDEX IF EXISTS admin.idx_contacts_email;
CREATE INDEX idx_contacts_email ON admin.contacts(email);
