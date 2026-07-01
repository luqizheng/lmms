-- Phase 8.1: multi-tenancy skeleton — Tenant entity + iam_users.tenant_id.

CREATE TABLE IF NOT EXISTS iam_tenants (
    id           BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    slug         VARCHAR(64)     NOT NULL,
    display_name VARCHAR(128)    NOT NULL,
    status       VARCHAR(16)     NOT NULL DEFAULT 'active',
    created_at   DATETIME        NOT NULL,
    updated_at   DATETIME        NOT NULL,
    UNIQUE KEY uk_tenant_slug (slug)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT INTO iam_tenants (id, slug, display_name, status, created_at, updated_at)
VALUES (1, 'default', 'Default Tenant', 'active', NOW(), NOW())
ON DUPLICATE KEY UPDATE id = id;

ALTER TABLE iam_users
    ADD COLUMN tenant_id BIGINT UNSIGNED NOT NULL DEFAULT 1 AFTER id;

ALTER TABLE iam_users
    ADD CONSTRAINT fk_iam_users_tenant FOREIGN KEY (tenant_id) REFERENCES iam_tenants(id);

ALTER TABLE iam_users
    DROP INDEX username;

ALTER TABLE iam_users
    ADD UNIQUE KEY uk_tenant_username (tenant_id, username);

INSERT IGNORE INTO iam_roles (id, name, code, description, data_scope, status, created_at, updated_at)
VALUES ('role-tenant-admin-seed', '租户管理员', 'tenant_admin', '租户管理员（Phase 8.1 全局共享 seed）', 'all', 'active', NOW(), NOW());
