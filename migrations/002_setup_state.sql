-- V1.0.1 SetupState 表：记录管理员初始化状态
CREATE TABLE IF NOT EXISTS setup_state (
    id INTEGER PRIMARY KEY,
    admin_initialized INTEGER NOT NULL DEFAULT 0,
    initialized_at TEXT
);

INSERT OR IGNORE INTO setup_state (id, admin_initialized) VALUES (1, 0);