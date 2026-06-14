-- Namespaces: app + env + profile
CREATE TABLE namespaces (
    id VARCHAR(36) PRIMARY KEY,
    app VARCHAR(128) NOT NULL COMMENT 'Application name',
    env VARCHAR(32) NOT NULL COMMENT 'Environment (dev/staging/prod)',
    profile VARCHAR(64) COMMENT 'Profile name',
    description VARCHAR(512) COMMENT 'Description',
    created_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    updated_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    INDEX idx_ns_app (app),
    INDEX idx_ns_env (env),
    UNIQUE KEY uk_ns_app_env_profile (app, env, profile)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Config Items: key-value pairs with type
CREATE TABLE config_items (
    id VARCHAR(36) PRIMARY KEY,
    namespace_id VARCHAR(36) NOT NULL COMMENT 'Namespace ID',
    key VARCHAR(256) NOT NULL COMMENT 'Config key',
    value TEXT NOT NULL COMMENT 'Config value',
    value_type VARCHAR(32) NOT NULL DEFAULT 'string' COMMENT 'Value type',
    is_secret TINYINT(1) NOT NULL DEFAULT 0 COMMENT 'Is secret',
    description VARCHAR(512) COMMENT 'Description',
    created_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    updated_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    INDEX idx_item_ns (namespace_id),
    INDEX idx_item_key (key),
    CONSTRAINT fk_item_ns FOREIGN KEY (namespace_id) REFERENCES namespaces(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Config Versions: audit trail for changes
CREATE TABLE config_versions (
    id VARCHAR(36) PRIMARY KEY,
    namespace_id VARCHAR(36) NOT NULL COMMENT 'Namespace ID',
    item_id VARCHAR(36) NOT NULL COMMENT 'Config item ID',
    key VARCHAR(256) NOT NULL COMMENT 'Config key',
    old_value TEXT NOT NULL COMMENT 'Old value',
    new_value TEXT NOT NULL COMMENT 'New value',
    operator VARCHAR(64) COMMENT 'Operator',
    comment VARCHAR(512) COMMENT 'Comment',
    created_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    INDEX idx_version_ns (namespace_id),
    INDEX idx_version_item (item_id),
    INDEX idx_version_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
