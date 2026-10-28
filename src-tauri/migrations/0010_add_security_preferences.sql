CREATE TABLE IF NOT EXISTS user_security_preferences (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  inactivity_timeout_minutes INTEGER NOT NULL DEFAULT 5,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_user_security_preferences_timeout
  ON user_security_preferences(inactivity_timeout_minutes);
