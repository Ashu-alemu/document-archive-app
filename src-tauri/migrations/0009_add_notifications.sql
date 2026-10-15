-- Notifications are retained indefinitely by default. Any future cleanup must
-- be an explicit administrator-controlled policy and must not touch audit logs.
CREATE TABLE IF NOT EXISTS notifications (
  id TEXT PRIMARY KEY,
  action TEXT NOT NULL,
  title TEXT NOT NULL,
  message TEXT NOT NULL,
  entity_type TEXT,
  entity_id TEXT,
  actor_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  severity TEXT NOT NULL DEFAULT 'info',
  created_at TEXT NOT NULL,
  read_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_notifications_recent
  ON notifications(created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_notifications_unread
  ON notifications(read_at, created_at DESC);
