-- Operation Logs: Business operation audit trail
CREATE TABLE audit_operation_logs (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    user_id VARCHAR(64) NOT NULL COMMENT 'User ID',
    action VARCHAR(128) NOT NULL COMMENT 'Action',
    resource VARCHAR(256) NOT NULL COMMENT 'Resource',
    resource_id VARCHAR(128) COMMENT 'Resource ID',
    before_value TEXT COMMENT 'Before value',
    after_value TEXT COMMENT 'After value',
    request_id VARCHAR(64) NOT NULL COMMENT 'Request ID',
    ip_address VARCHAR(45) COMMENT 'IP Address',
    user_agent TEXT COMMENT 'User-Agent',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_audit_operation_logs_user_id (user_id),
    INDEX idx_audit_operation_logs_action (action),
    INDEX idx_audit_operation_logs_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Login Logs: Authentication audit trail
CREATE TABLE audit_login_logs (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    user_id VARCHAR(64) COMMENT 'User ID',
    username VARCHAR(128) NOT NULL COMMENT 'Username',
    success TINYINT(1) NOT NULL COMMENT 'Success',
    fail_reason VARCHAR(256) COMMENT 'Fail reason',
    ip_address VARCHAR(45) NOT NULL COMMENT 'IP Address',
    location VARCHAR(256) COMMENT 'Location',
    user_agent TEXT COMMENT 'User-Agent',
    request_id VARCHAR(64) NOT NULL COMMENT 'Request ID',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_audit_login_logs_username (username),
    INDEX idx_audit_login_logs_success (success),
    INDEX idx_audit_login_logs_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- API Access Logs: Gateway access audit
CREATE TABLE audit_api_access_logs (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    service_name VARCHAR(64) NOT NULL COMMENT 'Service name',
    method VARCHAR(10) NOT NULL COMMENT 'HTTP method',
    path VARCHAR(512) NOT NULL COMMENT 'Request path',
    status_code INT NOT NULL COMMENT 'Status code',
    duration_ms INT NOT NULL COMMENT 'Duration ms',
    user_id VARCHAR(64) COMMENT 'User ID',
    request_id VARCHAR(64) NOT NULL COMMENT 'Request ID',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_audit_api_access_logs_service_name (service_name),
    INDEX idx_audit_api_access_logs_path (path(255)),
    INDEX idx_audit_api_access_logs_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Event idempotency table: consumer of iam events from RabbitMQ (Phase 4).
-- INSERT ... ON DUPLICATE KEY UPDATE on event_id makes consumption at-least-once safe.
CREATE TABLE IF NOT EXISTS audit_events (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    event_id        CHAR(36)     NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    occurred_at     DATETIME(3)  NOT NULL,
    received_at     DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    trace_id        VARCHAR(64)  NULL,
    producer        VARCHAR(64)  NOT NULL,
    payload_json    JSON         NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uk_audit_events_event_id (event_id),
    KEY idx_audit_events_type_time (event_type, occurred_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
