CREATE INDEX IF NOT EXISTS idx_audit_logs_created_action ON audit_logs(created_at DESC, action);
