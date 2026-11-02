CREATE TABLE IF NOT EXISTS department_roles (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT NOT NULL
);

INSERT OR IGNORE INTO department_roles (id, name, description) VALUES
  ('department_manager', 'Department Manager', 'Manage documents and memberships within assigned departments.'),
  ('department_contributor', 'Department Contributor', 'Create and edit permitted department documents.'),
  ('department_viewer', 'Department Viewer', 'View, search, and open permitted department documents.');

-- SQLite rejects ALTER TABLE ... ADD COLUMN when a REFERENCES column has a
-- non-NULL default. Rebuild this small table inside the migration instead.
-- The SQL plugin runs each migration transactionally, so a failed rebuild
-- rolls back without leaving a replacement table or partial data.
CREATE TABLE department_memberships_new (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  department_id TEXT NOT NULL REFERENCES departments(id) ON DELETE CASCADE,
  role_id TEXT NOT NULL REFERENCES department_roles(id),
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, department_id)
);

INSERT INTO department_memberships_new (user_id, department_id, role_id, active, created_at, updated_at)
SELECT dm.user_id,
       dm.department_id,
       CASE
         WHEN EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = dm.user_id AND ur.role_id IN ('admin', 'manager')) THEN 'department_manager'
         WHEN EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = dm.user_id AND ur.role_id = 'contributor') THEN 'department_contributor'
         ELSE 'department_viewer'
       END,
       dm.active,
       dm.created_at,
       dm.updated_at
FROM department_memberships dm;

DROP TABLE department_memberships;
ALTER TABLE department_memberships_new RENAME TO department_memberships;

CREATE INDEX IF NOT EXISTS idx_department_memberships_department ON department_memberships(department_id, active);
CREATE INDEX IF NOT EXISTS idx_department_memberships_user ON department_memberships(user_id, active);
CREATE INDEX IF NOT EXISTS idx_department_memberships_role ON department_memberships(role_id, active);
