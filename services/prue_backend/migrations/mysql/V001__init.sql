-- prue_backend 表结构。表前缀 `prue_`。

CREATE TABLE IF NOT EXISTS prue_menu_configs (
    id VARCHAR(64) PRIMARY KEY,
    version BIGINT NOT NULL,
    name VARCHAR(128) NOT NULL,
    description TEXT NULL,
    routes_json JSON NOT NULL,
    routes_etag CHAR(64) NOT NULL DEFAULT '',
    status VARCHAR(32) NOT NULL DEFAULT 'draft',
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    created_by VARCHAR(128) NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    published_at TIMESTAMP NULL,
    deleted_at TIMESTAMP NULL,
    UNIQUE KEY uk_prue_menu_configs_version (version),
    INDEX idx_prue_menu_configs_active (is_active),
    INDEX idx_prue_menu_configs_status (status)
);
