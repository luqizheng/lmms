-- Operation Logs: Business operation audit trail
CREATE TABLE IF NOT EXISTS audit_operation_logs (
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

CREATE INDEX IF NOT EXISTS idx_audit_operation_logs_user_id ON audit_operation_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_operation_logs_action ON audit_operation_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_operation_logs_created_at ON audit_operation_logs(created_at);

-- Login Logs: Authentication audit trail
CREATE TABLE IF NOT EXISTS audit_login_logs (
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

CREATE INDEX IF NOT EXISTS idx_audit_login_logs_username ON audit_login_logs(username);
CREATE INDEX IF NOT EXISTS idx_audit_login_logs_success ON audit_login_logs(success);
CREATE INDEX IF NOT EXISTS idx_audit_login_logs_created_at ON audit_login_logs(created_at);

-- API Access Logs: Gateway access audit
CREATE TABLE IF NOT EXISTS audit_api_access_logs (
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

CREATE INDEX IF NOT EXISTS idx_audit_api_access_logs_service_name ON audit_api_access_logs(service_name);
CREATE INDEX IF NOT EXISTS idx_audit_api_access_logs_path ON audit_api_access_logs(path);
CREATE INDEX IF NOT EXISTS idx_audit_api_access_logs_created_at ON audit_api_access_logs(created_at);

-- Event idempotency table: consumer of iam events from RabbitMQ (Phase 4).
CREATE TABLE IF NOT EXISTS audit_events (
    id              INTEGER      NOT NULL PRIMARY KEY AUTOINCREMENT,
    event_id        TEXT         NOT NULL UNIQUE,
    event_type      TEXT         NOT NULL,
    occurred_at     TEXT         NOT NULL,
    received_at     TEXT         NOT NULL DEFAULT (datetime('now')),
    trace_id        TEXT         NULL,
    producer        TEXT         NOT NULL,
    payload_json    TEXT         NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_events_type_time ON audit_events(event_type, occurred_at);
