SET search_path TO admin, public;

-- Seed the contact graph with known business relationships.

-- Organizations
INSERT INTO admin.contacts (contact_type, name, domain, metadata)
VALUES ('organization', 'Naked Pineapple', 'pineappleskinco.com', '{"internal": true}');

INSERT INTO admin.contacts (contact_type, name, domain, metadata)
VALUES ('organization', 'Intermountain Nutrition', 'intermountainnutrition.com', '{}');

-- People
INSERT INTO admin.contacts (contact_type, name, email, metadata)
VALUES ('person', 'Ry Fry', 'ryanfry2012@gmail.com', '{}');

INSERT INTO admin.contacts (contact_type, name, email, domain, metadata)
VALUES ('person', 'Dustin Rykert', 'drykert@intermountainnutrition.com', 'intermountainnutrition.com', '{}');

-- Relationships
-- Ry Fry → CEO_OF → Naked Pineapple
INSERT INTO admin.contact_relationships (from_contact_id, to_contact_id, relationship_type, properties)
SELECT p.id, o.id, 'ceo_of', '{}'::jsonb
FROM admin.contacts p, admin.contacts o
WHERE p.email = 'ryanfry2012@gmail.com' AND o.name = 'Naked Pineapple';

-- Intermountain Nutrition → SUPPLIES → Naked Pineapple
INSERT INTO admin.contact_relationships (from_contact_id, to_contact_id, relationship_type, properties)
SELECT s.id, np.id, 'supplies', '{"context": "pea protein isolate, rice protein"}'::jsonb
FROM admin.contacts s, admin.contacts np
WHERE s.name = 'Intermountain Nutrition' AND np.name = 'Naked Pineapple';

-- Dustin Rykert → WORKS_AT → Intermountain Nutrition
INSERT INTO admin.contact_relationships (from_contact_id, to_contact_id, relationship_type, properties)
SELECT p.id, o.id, 'works_at', '{}'::jsonb
FROM admin.contacts p, admin.contacts o
WHERE p.email = 'drykert@intermountainnutrition.com' AND o.name = 'Intermountain Nutrition';
