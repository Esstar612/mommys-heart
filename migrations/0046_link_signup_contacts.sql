INSERT INTO case_contacts (id, case_id, contact_id, role, note, is_primary, added_by)
SELECT
    'cc-' || nextval('app_id_seq'),
    c.id,
    ct.id,
    'client',
    '',
    NOT EXISTS (SELECT 1 FROM case_contacts p WHERE p.case_id = c.id AND p.is_primary),
    'Signup backfill'
FROM cases c
JOIN users u ON u.id = c.owner_id AND u.role = 'client'
JOIN contacts ct ON ct.user_id = u.id AND NOT ct.archived
WHERE NOT EXISTS (
    SELECT 1 FROM case_contacts x WHERE x.case_id = c.id AND x.role = 'client'
)
ON CONFLICT DO NOTHING;

INSERT INTO contacts (id, first_name, last_name, phone, types, source)
SELECT
    'ct-emergency-' || a.case_id,
    btrim(regexp_replace(btrim(a.emergency_name), '\s*\S+$', '')),
    regexp_replace(btrim(a.emergency_name), '^.*\s', ''),
    a.emergency_phone,
    ARRAY['emergency_contact'],
    'Client signup'
FROM client_agreements a
JOIN cases c ON c.id = a.case_id
WHERE btrim(a.emergency_name) <> ''
  AND NOT EXISTS (
      SELECT 1 FROM case_contacts x WHERE x.case_id = a.case_id AND x.role = 'emergency_contact'
  )
ON CONFLICT DO NOTHING;

INSERT INTO case_contacts (id, case_id, contact_id, role, note, is_primary, added_by)
SELECT
    'cc-' || nextval('app_id_seq'),
    a.case_id,
    ct.id,
    'emergency_contact',
    btrim(a.emergency_relationship),
    false,
    'Signup backfill'
FROM client_agreements a
JOIN contacts ct ON ct.id = 'ct-emergency-' || a.case_id
WHERE NOT EXISTS (
    SELECT 1 FROM case_contacts x WHERE x.case_id = a.case_id AND x.role = 'emergency_contact'
)
ON CONFLICT DO NOTHING;
