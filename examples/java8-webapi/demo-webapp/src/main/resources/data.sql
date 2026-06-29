-- Tasks: 3 seed
INSERT INTO task (title, description, status, assignee_id, created_at, updated_at) VALUES
  ('Onboard new dev',  'Set up workstation for new joiner',  'CREATED',     NULL,    CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
  ('Fix login bug',    'Investigate 401 on /v1/iam/auth/login', 'ASSIGNED',   '1001',  CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
  ('Write docs',       'Document Phase 5 architecture',         'IN_PROGRESS', '1002',  CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);