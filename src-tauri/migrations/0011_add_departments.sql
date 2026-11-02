CREATE TABLE IF NOT EXISTS departments (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL COLLATE NOCASE UNIQUE,
  description TEXT NOT NULL,
  icon TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO departments (id, name, description, icon, sort_order, created_at, updated_at) VALUES
  ('administration', 'Administration', 'Church leadership, records, and general administration.', '⌂', 1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('finance', 'Finance', 'Financial records, receipts, budgets, and reports.', '◈', 2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('youth', 'Youth', 'Youth ministry programs, activities, and records.', '✦', 3, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('sunday-school', 'Sunday School', 'Teaching resources, classes, and Sunday School records.', '✎', 4, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('choir', 'Choir', 'Choir schedules, music, and ministry documents.', '♫', 5, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('media', 'Media', 'Photography, video, communications, and media assets.', '◉', 6, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('spiritual-service', 'Spiritual Service', 'Worship, service programs, and spiritual ministry records.', '✚', 7, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));

CREATE TABLE IF NOT EXISTS department_memberships (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  department_id TEXT NOT NULL REFERENCES departments(id) ON DELETE CASCADE,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, department_id)
);

-- Preserve existing access without changing accounts. Administrators can enter
-- every department; existing non-administrators retain a safe Administration
-- entry point until an administrator assigns additional memberships.
INSERT OR IGNORE INTO department_memberships (user_id, department_id, active, created_at, updated_at)
SELECT u.id,
       CASE WHEN EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = u.id AND ur.role_id = 'admin')
            THEN d.id ELSE 'administration' END,
       1,
       strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
       strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM users u
JOIN departments d ON d.id = 'administration'
WHERE NOT EXISTS (
  SELECT 1 FROM user_roles ur WHERE ur.user_id = u.id AND ur.role_id = 'admin'
)
UNION ALL
SELECT u.id, d.id, 1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM users u
JOIN user_roles ur ON ur.user_id = u.id AND ur.role_id = 'admin'
JOIN departments d;

CREATE INDEX IF NOT EXISTS idx_departments_active_order ON departments(active, sort_order, name);
CREATE INDEX IF NOT EXISTS idx_department_memberships_department ON department_memberships(department_id, active);
CREATE INDEX IF NOT EXISTS idx_department_memberships_user ON department_memberships(user_id, active);
