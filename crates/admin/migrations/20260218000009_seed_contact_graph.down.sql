SET search_path TO admin, public;

DELETE FROM admin.contact_relationships
WHERE from_contact_id IN (
    SELECT id FROM admin.contacts
    WHERE email IN ('ryanfry2012@gmail.com', 'drykert@intermountainnutrition.com')
       OR (name = 'Pineapple Skin Co.' AND contact_type = 'organization')
       OR (name = 'Intermountain Nutrition' AND contact_type = 'organization')
)
OR to_contact_id IN (
    SELECT id FROM admin.contacts
    WHERE email IN ('ryanfry2012@gmail.com', 'drykert@intermountainnutrition.com')
       OR (name = 'Pineapple Skin Co.' AND contact_type = 'organization')
       OR (name = 'Intermountain Nutrition' AND contact_type = 'organization')
);

DELETE FROM admin.contacts
WHERE email IN ('ryanfry2012@gmail.com', 'drykert@intermountainnutrition.com')
   OR (name = 'Pineapple Skin Co.' AND contact_type = 'organization')
   OR (name = 'Intermountain Nutrition' AND contact_type = 'organization');
