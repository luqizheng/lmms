-- Operation Logs: Business operation audit trail
CREATE TABLE IF NOT EXISTS operation_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    resource TEXT NOT NULL,
    resource_id TEXT,
    before_value TEXT,
    after_value TEXT,
    request_id TEXT NOT NULL,
    ip_address TEXT,
    user_agent TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_operation_user_id ON operation_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_operation_action ON operation_logs(action);
CREATE INDEX IF NOT EXISTS idx_operation_created_at ON operation_logs(created_at);

-- Login Logs: Authentication audit trail
CREATE TABLE IF NOT EXISTS login_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT,
    username TEXT NOT NULL,
    success INTEGER NOT NULL,
    fail_reason TEXT,
    ip_address TEXT NOT NULL,
    location TEXT,
    user_agent TEXT,
    request_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_login_username ON login_logs(username);
CREATE INDEX IF NOT EXISTS idx_login_success ON login_logs(success);
CREATE INDEX IF NOT EXISTS idx_login_created_at ON login_logs(created_at);

-- API Access Logs: Gateway access audit
CREATE TABLE IF NOT EXISTS api_access_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL,
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    user_id TEXT,
    request_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_api_service ON api_access_logs(service_name);
CREATE INDEX IF NOT EXISTS idx_api_path ON api_access_logs(path);
CREATE INDEX IF NOT EXISTS idx_api_created_at ON api_access_logs(created_at);
