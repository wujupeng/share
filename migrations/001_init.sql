CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'readonly' CHECK(role IN ('admin', 'readwrite', 'readonly')),
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'locked', 'disabled')),
    login_fail_count INTEGER NOT NULL DEFAULT 0,
    locked_until TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE IF NOT EXISTS permission_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('admin', 'readwrite', 'readonly')),
    allowed_actions TEXT NOT NULL DEFAULT '[]',
    inherit INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE IF NOT EXISTS audit_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operator TEXT NOT NULL,
    action_type TEXT NOT NULL CHECK(action_type IN ('login', 'logout', 'upload', 'download', 'delete', 'rename', 'move', 'copy', 'mkdir', 'permission_change', 'user_manage')),
    target_path TEXT,
    detail TEXT,
    source_ip TEXT NOT NULL,
    result TEXT NOT NULL CHECK(result IN ('success', 'failure')),
    failure_reason TEXT,
    action_time TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE IF NOT EXISTS upload_tasks (
    id TEXT PRIMARY KEY,
    file_name TEXT NOT NULL,
    target_path TEXT NOT NULL,
    total_size INTEGER NOT NULL,
    chunk_size INTEGER NOT NULL,
    total_chunks INTEGER NOT NULL,
    uploaded_chunks TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'uploading', 'merging', 'completed', 'failed', 'cancelled')),
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_status ON users(status);
CREATE INDEX IF NOT EXISTS idx_permission_rules_path ON permission_rules(path);
CREATE INDEX IF NOT EXISTS idx_permission_rules_role ON permission_rules(role);
CREATE INDEX IF NOT EXISTS idx_audit_logs_operator ON audit_logs(operator);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action_type ON audit_logs(action_type);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action_time ON audit_logs(action_time);
CREATE INDEX IF NOT EXISTS idx_audit_logs_composite ON audit_logs(operator, action_type, action_time);
CREATE INDEX IF NOT EXISTS idx_upload_tasks_status ON upload_tasks(status);
-- 版本化迁移记录表
CREATE TABLE IF NOT EXISTS schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
